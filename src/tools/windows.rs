use std::{io, os::windows::io::AsRawHandle, process::Child, time::Duration};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, LocalFree},
    Security::{
        Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    },
    System::{
        Diagnostics::ToolHelp::*, JobObjects::*, RemoteDesktop::*, SystemInformation::GetTickCount,
        Threading::*,
    },
    UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
};

pub struct Handle(pub HANDLE);
unsafe impl Send for Handle {}
impl Handle {
    pub fn new(value: HANDLE) -> io::Result<Self> {
        if value.is_null() || value == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub fn user_sid() -> io::Result<String> {
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = Handle::new(token)?;
        let mut size = 0;
        GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut size);
        let mut data = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        if GetTokenInformation(
            token.0,
            TokenUser,
            data.as_mut_ptr().cast(),
            size,
            &mut size,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let user = &*data.as_ptr().cast::<TOKEN_USER>();
        let mut text = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut length = 0;
        while *text.add(length) != 0 {
            length += 1;
        }
        let result = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
        LocalFree(text.cast());
        Ok(result)
    }
}

pub fn wide(text: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    text.as_ref().encode_wide().chain(Some(0)).collect()
}

pub struct Job(Handle);
impl Job {
    pub fn attach(child: &Child) -> io::Result<Self> {
        unsafe {
            let job = Handle::new(CreateJobObjectW(std::ptr::null(), std::ptr::null()))?;
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            ) == 0
                || AssignProcessToJobObject(job.0, child.as_raw_handle()) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let snapshot = Handle::new(CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0))?;
            let mut entry: THREADENTRY32 = std::mem::zeroed();
            entry.dwSize = size_of_val(&entry) as u32;
            let mut present = Thread32First(snapshot.0, &mut entry);
            while present != 0 {
                if entry.th32OwnerProcessID == child.id() {
                    let thread =
                        Handle::new(OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID))?;
                    if ResumeThread(thread.0) == u32::MAX {
                        return Err(io::Error::last_os_error());
                    }
                    return Ok(Self(job));
                }
                present = Thread32Next(snapshot.0, &mut entry);
            }
            Err(io::Error::other("无法恢复 Codex 主线程"))
        }
    }
    pub fn terminate(&self) {
        unsafe {
            TerminateJobObject(self.0.0, 1);
        }
    }
}

pub fn idle_duration() -> Option<Duration> {
    unsafe {
        let mut session = 0;
        if ProcessIdToSessionId(GetCurrentProcessId(), &mut session) == 0
            || session != WTSGetActiveConsoleSessionId()
        {
            return None;
        }
        let mut buffer = std::ptr::null_mut();
        let mut bytes = 0;
        if WTSQuerySessionInformationW(
            std::ptr::null_mut(),
            session,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        ) == 0
        {
            return None;
        }
        let valid = if bytes as usize >= size_of::<WTSINFOEXW>() {
            let info = &*buffer.cast::<WTSINFOEXW>();
            info.Level == 1
                && info.Data.WTSInfoExLevel1.SessionState == WTSActive
                && info.Data.WTSInfoExLevel1.SessionFlags == WTS_SESSIONSTATE_UNLOCK as i32
        } else {
            false
        };
        WTSFreeMemory(buffer.cast());
        if !valid {
            return None;
        }
        let mut input = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        (GetLastInputInfo(&mut input) != 0)
            .then(|| Duration::from_millis(u64::from(GetTickCount().wrapping_sub(input.dwTime))))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn identity_and_utf16_are_native_and_stable() {
        let sid = super::user_sid().unwrap();
        assert!(sid.starts_with("S-1-"));
        assert_eq!(sid, super::user_sid().unwrap());
        assert_eq!(super::wide("中文").last(), Some(&0));
    }
}
