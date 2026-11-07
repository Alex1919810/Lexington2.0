//! Explicit requests from local context-menu integrations. No clipboard polling.
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

pub const MAX_MESSAGE: usize = 1024 * 1024;

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Request {
    pub text: String,
    pub files: Vec<String>,
    pub source: String,
    pub line: Option<u32>,
}

impl Request {
    pub fn validate(&self) -> io::Result<()> {
        if self.text.trim().is_empty() && self.files.is_empty() {
            return Err(io::Error::other("没有选中文字或文件"));
        }
        if self.files.len() > 32
            || self
                .files
                .iter()
                .any(|path| !std::path::Path::new(path).is_absolute() || path.contains('\0'))
        {
            return Err(io::Error::other("请选择不超过 32 个本地文件（绝对路径）"));
        }
        if serde_json::to_vec(self)?.len() > MAX_MESSAGE {
            return Err(io::Error::other("选中内容超过 1 MiB，请缩小选区"));
        }
        Ok(())
    }

    pub fn prompt(&self) -> String {
        // JSON preserves filename boundaries, quotes and newlines without shell parsing.
        format!(
            "看看Lexington怎么说：请阅读以下选中文字及其来源，或查看指定文件，给出你的分析与建议。\n{}",
            serde_json::to_string_pretty(self).expect("request serialization")
        )
    }
}

fn read_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut size = [0_u8; 4];
    if reader.read(&mut size[..1])? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut size[1..])?;
    let size = u32::from_le_bytes(size) as usize;
    if size == 0 || size > MAX_MESSAGE {
        return Err(io::Error::other("请求大小无效，最多 1 MiB"));
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

fn write_frame(writer: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.is_empty() || bytes.len() > MAX_MESSAGE {
        return Err(io::Error::other("请求大小无效，最多 1 MiB"));
    }
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()
}

#[cfg(target_os = "linux")]
mod local {
    use super::*;
    use std::{
        fs::{self, File, OpenOptions},
        os::{
            fd::AsRawFd,
            unix::{
                fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
                net::{UnixListener, UnixStream},
            },
        },
        path::{Path, PathBuf},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver},
        },
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };

    pub struct Endpoint {
        listener: UnixListener,
        path: PathBuf,
        _lock: File,
    }

    pub struct Inbox {
        requests: Receiver<Request>,
        stop: Arc<AtomicBool>,
        worker: Option<JoinHandle<()>>,
    }

    fn directory(dev: bool) -> PathBuf {
        let root = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        root.join(format!(
            "lexington-{}{}",
            unsafe { libc::geteuid() },
            if dev { "-dev" } else { "" }
        ))
    }

    impl Endpoint {
        pub fn bind(dev: bool) -> io::Result<Option<Self>> {
            Self::bind_at(&directory(dev))
        }

        pub(crate) fn bind_at(directory: &Path) -> io::Result<Option<Self>> {
            use std::os::unix::fs::DirBuilderExt;
            match fs::DirBuilder::new().mode(0o700).create(directory) {
                Ok(()) => (),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
                Err(error) => return Err(error),
            }
            let metadata = fs::symlink_metadata(directory)?;
            if !metadata.is_dir()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.mode() & 0o077 != 0
            {
                return Err(io::Error::other(
                    "Lexington 通信目录必须归当前用户所有且权限为 0700",
                ));
            }
            let lock = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(directory.join("instance.lock"))?;
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                let error = io::Error::last_os_error();
                return if error.kind() == io::ErrorKind::WouldBlock {
                    Ok(None)
                } else {
                    Err(error)
                };
            }
            let path = directory.join("inbox.sock");
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_socket() => fs::remove_file(&path)?,
                Ok(_) => return Err(io::Error::other("Lexington 通信路径被其他文件占用")),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(error),
            }
            let listener = UnixListener::bind(&path)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
            listener.set_nonblocking(true)?;
            Ok(Some(Self {
                listener,
                path,
                _lock: lock,
            }))
        }

        pub fn start(self, wake: impl Fn() + Send + 'static) -> Inbox {
            let (send, requests) = mpsc::sync_channel(16);
            let stop = Arc::new(AtomicBool::new(false));
            let stopping = stop.clone();
            let worker = thread::spawn(move || {
                while !stopping.load(Ordering::Relaxed) {
                    match self.listener.accept() {
                        Ok((mut stream, _)) => {
                            let result = (|| -> io::Result<()> {
                                // Only same-user local processes may enqueue a request.
                                let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
                                let mut length =
                                    std::mem::size_of::<libc::ucred>() as libc::socklen_t;
                                let status = unsafe {
                                    libc::getsockopt(
                                        stream.as_raw_fd(),
                                        libc::SOL_SOCKET,
                                        libc::SO_PEERCRED,
                                        (&mut credentials as *mut libc::ucred).cast(),
                                        &mut length,
                                    )
                                };
                                if status != 0 || credentials.uid != unsafe { libc::geteuid() } {
                                    return Err(io::Error::other("拒绝其他用户的请求"));
                                }
                                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                                stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                                let bytes = read_frame(&mut stream)?
                                    .ok_or_else(|| io::Error::other("空请求"))?;
                                let request: Request = serde_json::from_slice(&bytes)?;
                                request.validate()?;
                                send.try_send(request).map_err(|_| {
                                    io::Error::other("Lexington 请求队列已满或已关闭，请稍后再试")
                                })?;
                                wake();
                                Ok(())
                            })();
                            let response = match result {
                                Ok(()) => serde_json::json!({"ok": true}),
                                Err(error) => {
                                    serde_json::json!({"ok": false, "error": error.to_string()})
                                }
                            };
                            let _ = write_frame(&mut stream, &response);
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(25));
                        }
                        Err(_) => break,
                    }
                }
                // The instance lock remains held until after the socket is removed.
                let _ = fs::remove_file(&self.path);
                drop(self);
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

    fn send(mut stream: UnixStream, request: &Request) -> io::Result<()> {
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        write_frame(&mut stream, request)?;
        let response: serde_json::Value = serde_json::from_slice(
            &read_frame(&mut stream)?.ok_or_else(|| io::Error::other("Lexington 未确认请求"))?,
        )?;
        if response["ok"] == true {
            Ok(())
        } else {
            Err(io::Error::other(
                response["error"].as_str().unwrap_or("Lexington 拒绝了请求"),
            ))
        }
    }

    pub fn deliver(request: Request) -> io::Result<()> {
        request.validate()?;
        let path = directory(false).join("inbox.sock");
        match UnixStream::connect(&path) {
            Ok(stream) => return send(stream, &request),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) => {}
            Err(error) => return Err(error),
        }
        let mut child = std::process::Command::new(std::env::current_exe()?)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match UnixStream::connect(&path) {
                Ok(stream) => {
                    // Reap auto-started GUI children if this host stays alive.
                    thread::spawn(move || {
                        let _ = child.wait();
                    });
                    // Never retry once sent: a lost acknowledgement must not duplicate a task.
                    return send(stream, &request);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                    ) => {}
                Err(error) => return Err(error),
            }
            if let Some(status) = child.try_wait()?
                && !status.success()
            {
                return Err(io::Error::other("Lexington 启动失败，请先从桌面打开程序"));
            }
            if Instant::now() >= deadline {
                return Err(io::Error::other("等待 Lexington 启动超时"));
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn inbox_is_exclusive_bounded_fifo_and_recovers_stale_socket() {
            let path = std::env::temp_dir().join(format!(
                "lexington-ipc-test-{}-{}",
                std::process::id(),
                rand::random::<u64>()
            ));
            let endpoint = Endpoint::bind_at(&path).unwrap().unwrap();
            assert!(Endpoint::bind_at(&path).unwrap().is_none());
            let inbox = endpoint.start(|| {});
            for n in 0..16 {
                let request = Request {
                    text: format!("选区 {n}"),
                    files: vec!["/tmp/空 格\n$(不执行).rs".into()],
                    ..Default::default()
                };
                send(
                    UnixStream::connect(path.join("inbox.sock")).unwrap(),
                    &request,
                )
                .unwrap();
            }
            assert!(
                send(
                    UnixStream::connect(path.join("inbox.sock")).unwrap(),
                    &Request {
                        text: "overflow".into(),
                        ..Default::default()
                    }
                )
                .is_err()
            );
            for n in 0..16 {
                assert_eq!(inbox.poll().unwrap().text, format!("选区 {n}"));
            }
            assert!(inbox.poll().is_none());
            // Oversized frame is rejected before allocating or queuing content.
            let mut stream = UnixStream::connect(path.join("inbox.sock")).unwrap();
            stream
                .write_all(&((MAX_MESSAGE + 1) as u32).to_le_bytes())
                .unwrap();
            let response: serde_json::Value =
                serde_json::from_slice(&read_frame(&mut stream).unwrap().unwrap()).unwrap();
            assert_eq!(response["ok"], false);
            drop(inbox);
            assert!(!path.join("inbox.sock").exists());
            let stale = UnixListener::bind(path.join("inbox.sock")).unwrap();
            drop(stale);
            drop(Endpoint::bind_at(&path).unwrap().unwrap().start(|| {}));
            fs::remove_dir_all(path).unwrap();
        }

        #[test]
        fn inbox_refuses_public_or_symlinked_directories() {
            let path = std::env::temp_dir()
                .join(format!("lexington-insecure-test-{}", rand::random::<u64>()));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(Endpoint::bind_at(&path).is_err());
            let link = path.with_extension("link");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(Endpoint::bind_at(&link).is_err());
            fs::remove_file(link).unwrap();
            fs::remove_dir(path).unwrap();
        }
    }
}

#[cfg(target_os = "linux")]
pub use local::{Endpoint, Inbox, deliver};

#[cfg(windows)]
#[path = "handoff_windows.rs"]
mod windows;
#[cfg(all(windows, test))]
pub(crate) use windows::deliver_at;
#[cfg(windows)]
pub use windows::{Endpoint, Inbox, deliver};

#[cfg(not(any(target_os = "linux", windows)))]
pub fn deliver(_request: Request) -> io::Result<()> {
    Err(io::Error::other("右键集成暂仅支持 Linux"))
}

pub fn native_host() -> io::Result<()> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    while let Some(bytes) = read_frame(&mut input)? {
        let result = serde_json::from_slice::<Request>(&bytes)
            .map_err(io::Error::from)
            .and_then(deliver);
        let response = match result {
            Ok(()) => serde_json::json!({"ok": true}),
            Err(error) => serde_json::json!({"ok": false, "error": error.to_string()}),
        };
        write_frame(&mut output, &response)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_preserve_sources_and_reject_empty_relative_or_oversized_input() {
        let request = Request {
            text: "你好\n第二行".into(),
            files: vec![
                std::env::temp_dir()
                    .join("空 格.rs")
                    .to_string_lossy()
                    .into_owned(),
            ],
            source: "VS Code".into(),
            line: Some(12),
        };
        request.validate().unwrap();
        let prompt = request.prompt();
        let saved: Request = serde_json::from_str(prompt.split_once('\n').unwrap().1).unwrap();
        assert_eq!(saved, request);
        assert!(Request::default().validate().is_err());
        assert!(
            Request {
                files: vec!["relative.rs".into()],
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Request {
                text: "x".repeat(MAX_MESSAGE),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        let mut framed = Vec::new();
        write_frame(&mut framed, &request).unwrap();
        let decoded: Request =
            serde_json::from_slice(&read_frame(&mut framed.as_slice()).unwrap().unwrap()).unwrap();
        assert_eq!(decoded, request);
        assert!(read_frame(&mut &framed[..framed.len() - 1]).is_err());
    }
}
