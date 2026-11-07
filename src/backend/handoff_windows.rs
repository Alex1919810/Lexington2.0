use super::{MAX_MESSAGE, Request, read_frame, write_frame};
use crate::tools::windows::{Handle, user_sid, wide};
use std::{
    fs::File,
    io::{self, Read, Write},
    os::windows::io::{AsRawHandle, FromRawHandle},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_NO_DATA, ERROR_PIPE_BUSY,
        ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING, ERROR_PIPE_NOT_CONNECTED, GENERIC_READ,
        GENERIC_WRITE, LocalFree,
    },
    Security::{
        Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1},
        SECURITY_ATTRIBUTES,
    },
    Storage::FileSystem::{
        CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
    },
    System::Pipes::*,
};

fn name(dev: bool) -> io::Result<Vec<u16>> {
    Ok(wide(format!(
        r"\\.\pipe\lexington-{}-{}",
        user_sid()?,
        if dev { "dev" } else { "normal" }
    )))
}

struct Connection {
    file: File,
    deadline: Instant,
    stop: Arc<AtomicBool>,
}
impl Connection {
    fn retry<T>(&mut self, mut operation: impl FnMut(&mut File) -> io::Result<T>) -> io::Result<T> {
        loop {
            match operation(&mut self.file) {
                Err(error) if matches!(error.raw_os_error(), Some(code) if code == ERROR_NO_DATA as i32 || code == ERROR_PIPE_LISTENING as i32) =>
                {
                    if self.stop.load(Ordering::Relaxed) || Instant::now() >= self.deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "Lexington 通信超时",
                        ));
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                result => return result,
            }
        }
    }
}
impl Read for Connection {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.retry(|file| {
            let mut available = 0;
            if unsafe {
                PeekNamedPipe(
                    file.as_raw_handle(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::BrokenPipe {
                    return Ok(0);
                }
                return Err(error);
            }
            if available == 0 && !bytes.is_empty() {
                return Err(io::Error::from_raw_os_error(ERROR_NO_DATA as i32));
            }
            file.read(bytes)
        })
    }
}
impl Write for Connection {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.retry(|file| file.write(bytes))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Endpoint {
    file: File,
}
pub struct Inbox {
    requests: mpsc::Receiver<Request>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Endpoint {
    pub fn bind(dev: bool) -> io::Result<Option<Self>> {
        Self::bind_named(&name(dev)?)
    }
    #[cfg(test)]
    pub(crate) fn bind_at(path: &std::path::Path) -> io::Result<Option<Self>> {
        Self::bind_named(&test_name(path)?)
    }
    fn bind_named(name: &[u16]) -> io::Result<Option<Self>> {
        let sddl = wide(format!("D:P(A;;GA;;;{})(A;;GA;;;SY)", user_sid()?));
        unsafe {
            let mut descriptor = std::ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let attributes = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            };
            let handle = CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                (MAX_MESSAGE + 4) as u32,
                (MAX_MESSAGE + 4) as u32,
                0,
                &attributes,
            );
            let result = Handle::new(handle);
            LocalFree(descriptor);
            match result {
                Ok(handle) => {
                    let file = File::from_raw_handle(handle.0);
                    std::mem::forget(handle);
                    Ok(Some(Self { file }))
                }
                Err(error) if matches!(error.raw_os_error(), Some(code) if code == ERROR_ACCESS_DENIED as i32 || code == ERROR_PIPE_BUSY as i32) => {
                    Ok(None)
                }
                Err(error) => Err(error),
            }
        }
    }
    pub fn start(self, wake: impl Fn() + Send + 'static) -> Inbox {
        let (send, requests) = mpsc::sync_channel(16);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let worker = thread::spawn(move || {
            let mut connection = Connection {
                file: self.file,
                deadline: Instant::now(),
                stop: stopping.clone(),
            };
            while !stopping.load(Ordering::Relaxed) {
                let connected = unsafe {
                    ConnectNamedPipe(connection.file.as_raw_handle(), std::ptr::null_mut())
                } != 0;
                let code = io::Error::last_os_error().raw_os_error();
                if !connected && code != Some(ERROR_PIPE_CONNECTED as i32) {
                    if code == Some(ERROR_NO_DATA as i32) {
                        unsafe {
                            DisconnectNamedPipe(connection.file.as_raw_handle());
                        }
                    }
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                connection.deadline = Instant::now() + Duration::from_secs(2);
                let result = (|| -> io::Result<()> {
                    let bytes =
                        read_frame(&mut connection)?.ok_or_else(|| io::Error::other("空请求"))?;
                    let request: Request = serde_json::from_slice(&bytes)?;
                    request.validate()?;
                    send.try_send(request)
                        .map_err(|_| io::Error::other("Lexington 请求队列已满或已关闭"))?;
                    wake();
                    Ok(())
                })();
                let response = match result {
                    Ok(()) => serde_json::json!({"ok":true}),
                    Err(error) => serde_json::json!({"ok":false,"error":error.to_string()}),
                };
                let _ = write_frame(&mut connection, &response);
                while !stopping.load(Ordering::Relaxed) && Instant::now() < connection.deadline {
                    if unsafe {
                        PeekNamedPipe(
                            connection.file.as_raw_handle(),
                            std::ptr::null_mut(),
                            0,
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                        )
                    } == 0
                    {
                        break;
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                unsafe {
                    DisconnectNamedPipe(connection.file.as_raw_handle());
                }
            }
        });
        Inbox {
            requests,
            stop,
            worker: Some(worker),
        }
    }
}
impl Inbox {
    pub fn poll(&self) -> Option<Request> {
        self.requests.try_recv().ok()
    }
}
impl Drop for Inbox {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn open(name: &[u16]) -> io::Result<Connection> {
    unsafe {
        let handle = Handle::new(CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        ))?;
        let mode = PIPE_READMODE_BYTE | PIPE_NOWAIT;
        if SetNamedPipeHandleState(handle.0, &mode, std::ptr::null(), std::ptr::null()) == 0 {
            return Err(io::Error::last_os_error());
        }
        let file = File::from_raw_handle(handle.0);
        std::mem::forget(handle);
        Ok(Connection {
            file,
            deadline: Instant::now() + Duration::from_secs(3),
            stop: Arc::new(AtomicBool::new(false)),
        })
    }
}
fn send(mut connection: Connection, request: &Request) -> io::Result<()> {
    write_frame(&mut connection, request)?;
    let bytes =
        read_frame(&mut connection)?.ok_or_else(|| io::Error::other("Lexington 未确认请求"))?;
    let response: serde_json::Value = serde_json::from_slice(&bytes)?;
    if response["ok"] == true {
        Ok(())
    } else {
        Err(io::Error::other(
            response["error"].as_str().unwrap_or("Lexington 拒绝请求"),
        ))
    }
}
pub fn deliver(request: Request) -> io::Result<()> {
    request.validate()?;
    let name = name(false)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut started = false;
    loop {
        match open(&name) {
            Ok(connection) => return send(connection, &request),
            Err(error) if error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) && !started => {
                let executable = gui_executable(std::env::current_exe()?);
                let mut child = std::process::Command::new(executable)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()?;
                thread::spawn(move || {
                    let _ = child.wait();
                });
                started = true;
            }
            Err(error) if matches!(error.raw_os_error(), Some(code) if code == ERROR_FILE_NOT_FOUND as i32 || code == ERROR_PIPE_BUSY as i32 || code == ERROR_PIPE_NOT_CONNECTED as i32) =>
                {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "等待 Lexington 启动超时",
            ));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn gui_executable(executable: std::path::PathBuf) -> std::path::PathBuf {
    if executable
        .file_stem()
        .is_some_and(|stem| stem == "lexington-native-host")
    {
        executable.with_file_name("lexington.exe")
    } else {
        executable
    }
}

#[cfg(test)]
fn test_name(path: &std::path::Path) -> io::Result<Vec<u16>> {
    Ok(wide(format!(
        r"\\.\pipe\lexington-test-{}-{}",
        user_sid()?,
        path.file_name().unwrap().to_string_lossy()
    )))
}

#[cfg(test)]
pub(crate) fn deliver_at(path: &std::path::Path, request: &Request) -> io::Result<()> {
    let name = test_name(path)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match open(&name) {
            Ok(connection) => return send(connection, request),
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn connect(name: &[u16]) -> Connection {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Ok(connection) = open(name) {
                return connection;
            }
            assert!(Instant::now() < deadline, "test pipe unavailable");
            thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn native_host_launches_gui_instead_of_recursing() {
        let root = std::env::temp_dir();
        assert_eq!(
            gui_executable(root.join("lexington-native-host.exe")),
            root.join("lexington.exe")
        );
        assert_eq!(
            gui_executable(root.join("lexington.exe")),
            root.join("lexington.exe")
        );
    }
    #[test]
    fn private_pipe_is_exclusive_fifo_and_releases_on_shutdown() {
        let name = wide(format!(
            r"\\.\pipe\lexington-test-{}-{}",
            user_sid().unwrap(),
            rand::random::<u64>()
        ));
        let endpoint = Endpoint::bind_named(&name).unwrap().unwrap();
        assert!(Endpoint::bind_named(&name).unwrap().is_none());
        let inbox = endpoint.start(|| {});
        for index in 0..16 {
            let request = Request {
                text: format!("selection {index}"),
                ..Default::default()
            };
            send(connect(&name), &request).unwrap();
        }
        assert!(
            send(
                connect(&name),
                &Request {
                    text: "overflow".into(),
                    ..Default::default()
                }
            )
            .is_err()
        );
        for index in 0..16 {
            assert_eq!(inbox.poll().unwrap().text, format!("selection {index}"));
        }
        assert!(inbox.poll().is_none());
        drop(inbox);
        assert!(Endpoint::bind_named(&name).unwrap().is_some());
    }
    #[test]
    fn invalid_frames_disconnects_and_partial_reads_do_not_poison_or_hang_server() {
        assert_ne!(name(false).unwrap(), name(true).unwrap());
        let path = std::env::temp_dir().join(format!("pipe-malformed-{}", rand::random::<u64>()));
        let name = test_name(&path).unwrap();
        let inbox = Endpoint::bind_at(&path).unwrap().unwrap().start(|| {});
        drop(connect(&name));
        for length in [0, MAX_MESSAGE as u32 + 1] {
            let mut client = connect(&name);
            client.write_all(&length.to_le_bytes()).unwrap();
            let bytes = read_frame(&mut client).unwrap().unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["ok"],
                false
            );
        }
        assert!(send(connect(&name), &Request::default()).is_err());
        let request = Request {
            text: "after malformed clients".into(),
            ..Default::default()
        };
        send(connect(&name), &request).unwrap();
        assert_eq!(inbox.poll(), Some(request));
        let mut partial = connect(&name);
        partial.write_all(&[40, 0]).unwrap();
        thread::sleep(Duration::from_millis(40));
        let started = Instant::now();
        drop(inbox);
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
