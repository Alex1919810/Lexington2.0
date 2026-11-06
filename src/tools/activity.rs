//! System idle duration only; never subscribes to keyboard contents or pointer events.
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub struct Sample {
    pub idle: Option<Duration>,
    pub status: String,
    pub sampled: Instant,
}
pub struct Monitor {
    samples: mpsc::Receiver<Sample>,
    stop: mpsc::Sender<()>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Monitor {
    pub fn start(wake: impl Fn() + Send + 'static) -> Self {
        let (send, samples) = mpsc::channel();
        let (stop, stopped) = mpsc::channel();
        let worker = thread::spawn(move || {
            loop {
                let sample = sample();
                if send.send(sample).is_err() {
                    break;
                }
                wake();
                if stopped.recv_timeout(Duration::from_secs(2))
                    != Err(mpsc::RecvTimeoutError::Timeout)
                {
                    break;
                }
            }
        });
        Self {
            samples,
            stop,
            worker: Some(worker),
        }
    }
    pub fn poll(&self) -> Vec<Sample> {
        self.samples.try_iter().collect()
    }
    #[cfg(test)]
    pub fn mock(sample: Sample) -> Self {
        let (send, samples) = mpsc::channel();
        let (stop, _) = mpsc::channel();
        send.send(sample).unwrap();
        Self {
            samples,
            stop,
            worker: None,
        }
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(target_os = "linux")]
fn command(program: &str, args: &[&str]) -> Option<String> {
    use std::process::{Command, Stdio};
    // The UI never waits on subprocesses; enforce a bound even for broken D-Bus services.
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                return String::from_utf8(child.wait_with_output().ok()?.stdout).ok();
            }
            Ok(Some(_)) => return None,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn sample() -> Sample {
    let unavailable = || Sample {
        idle: None,
        status: "无法读取系统活动，可手动打卡".into(),
        sampled: Instant::now(),
    };
    let Some(session) = command(
        "loginctl",
        &[
            "show-session",
            "auto",
            "-p",
            "Active",
            "-p",
            "LockedHint",
            "-p",
            "Remote",
        ],
    ) else {
        return unavailable();
    };
    if !session.lines().any(|line| line == "Active=yes")
        || !session.lines().any(|line| line == "Remote=no")
    {
        return Sample {
            idle: None,
            status: "等待本地活动会话".into(),
            sampled: Instant::now(),
        };
    }
    if !session.lines().any(|line| line == "LockedHint=no") {
        return Sample {
            idle: None,
            status: "等待解锁".into(),
            sampled: Instant::now(),
        };
    }
    if let Some(reply) = command(
        "gdbus",
        &[
            "call",
            "--session",
            "--timeout",
            "2",
            "--dest",
            "org.gnome.Mutter.IdleMonitor",
            "--object-path",
            "/org/gnome/Mutter/IdleMonitor/Core",
            "--method",
            "org.gnome.Mutter.IdleMonitor.GetIdletime",
        ],
    ) && let Some(milliseconds) = reply
        .trim()
        .strip_prefix("(uint64 ")
        .and_then(|s| s.strip_suffix(",)"))
        .and_then(|s| s.parse::<u64>().ok())
    {
        return Sample {
            idle: Some(Duration::from_millis(milliseconds)),
            status: "正在检测工作活动".into(),
            sampled: Instant::now(),
        };
    }
    // XWayland's idle clock doesn't include native Wayland activity.
    if std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s == "wayland") {
        return unavailable();
    }
    use x11rb::{connection::Connection, protocol::screensaver::ConnectionExt};
    if let Ok((connection, screen)) = x11rb::connect(None)
        && let Ok(cookie) = connection.screensaver_query_info(connection.setup().roots[screen].root)
        && let Ok(reply) = cookie.reply()
    {
        return Sample {
            idle: Some(Duration::from_millis(u64::from(reply.ms_since_user_input))),
            status: "正在检测工作活动".into(),
            sampled: Instant::now(),
        };
    }
    unavailable()
}
#[cfg(not(target_os = "linux"))]
fn sample() -> Sample {
    Sample {
        idle: None,
        status: "当前平台支持手动打卡".into(),
        sampled: Instant::now(),
    }
}
