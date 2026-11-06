use super::config::ClockConfig;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub day: String,
    pub hour: u32,
    pub minute: u32,
}
impl LocalTime {
    pub fn label(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}
pub fn local_time(timestamp: i64) -> Option<LocalTime> {
    #[cfg(unix)]
    {
        let native = timestamp as libc::time_t;
        if native as i128 != i128::from(timestamp) {
            return None;
        }
        let timestamp = native;
        let mut date = std::mem::MaybeUninit::<libc::tm>::uninit();
        // localtime_r initializes tm using the process local timezone.
        if unsafe { libc::localtime_r(&timestamp, date.as_mut_ptr()) }.is_null() {
            return None;
        }
        let date = unsafe { date.assume_init() };
        Some(LocalTime {
            day: format!(
                "{:04}-{:02}-{:02}",
                date.tm_year + 1900,
                date.tm_mon + 1,
                date.tm_mday
            ),
            hour: date.tm_hour as u32,
            minute: date.tm_min as u32,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = timestamp;
        None
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    day: String,
    started: Option<i64>,
    handled: bool,
    reminded: bool,
}
struct Candidate {
    started: i64,
    since: Instant,
}
#[derive(Default)]
pub struct ClockState {
    record: Record,
    path: Option<PathBuf>,
    candidate: Option<Candidate>,
    last_sample: Option<Instant>,
    last_input: Option<Instant>,
}
impl ClockState {
    pub fn load(path: PathBuf, now: i64) -> Result<Self, String> {
        let record = match fs::read(&path) {
            Ok(bytes) => {
                let record: Record = serde_json::from_slice(&bytes)
                    .map_err(|_| "打卡记录损坏，未覆盖原文件。".to_owned())?;
                if record
                    .started
                    .is_some_and(|start| local_time(start).is_none_or(|t| t.day != record.day))
                    || (record.started.is_some() && !record.handled)
                    || (record.reminded && record.started.is_none())
                    || record.day.len() != 10
                {
                    return Err("打卡记录无效，未覆盖原文件。".into());
                }
                record
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Record::default(),
            Err(e) => return Err(format!("读取打卡记录失败：{e}")),
        };
        let mut state = Self {
            record,
            path: Some(path),
            ..Self::default()
        };
        state.roll_day(now)?;
        Ok(state)
    }
    fn store(&mut self, record: Record) -> Result<(), String> {
        if let Some(path) = &self.path {
            use std::io::Write;
            let parent = path.parent().ok_or("打卡记录路径无效")?;
            fs::create_dir_all(parent).map_err(|e| format!("保存打卡记录失败：{e}"))?;
            let temporary = path.with_extension(format!("{}.tmp", rand::random::<u64>()));
            let mut options = fs::OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temporary)
                .map_err(|e| format!("保存打卡记录失败：{e}"))?;
            let bytes = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
            let result = file.write_all(&bytes).and_then(|_| file.sync_all());
            drop(file);
            if let Err(e) = result.and_then(|_| fs::rename(&temporary, path)) {
                let _ = fs::remove_file(temporary);
                return Err(format!("保存打卡记录失败：{e}"));
            }
        }
        self.record = record;
        Ok(())
    }
    fn roll_day(&mut self, now: i64) -> Result<(), String> {
        let day = local_time(now).ok_or("无法读取本地日期")?.day;
        if self.record.day != day {
            self.store(Record {
                day,
                ..Record::default()
            })?;
            self.reset_candidate();
        }
        Ok(())
    }
    pub fn started(&self) -> Option<i64> {
        self.record.started
    }
    pub fn running(&self) -> bool {
        self.record.started.is_some() && !self.record.reminded
    }
    pub fn reset_candidate(&mut self) {
        self.candidate = None;
        self.last_sample = None;
        self.last_input = None;
    }
    pub fn toggle(&mut self, config: &ClockConfig, now: i64) -> Result<String, String> {
        self.roll_day(now)?;
        let cancel = self.running();
        self.store(Record {
            day: self.record.day.clone(),
            started: (!cancel).then_some(now),
            handled: true,
            reminded: false,
        })?;
        self.reset_candidate();
        Ok(if cancel {
            config.clock_in_cancel_message.clone()
        } else {
            config.clock_in_message.clone()
        })
    }
    pub fn undo(&mut self, now: i64) -> Result<(), String> {
        self.roll_day(now)?;
        self.store(Record {
            day: self.record.day.clone(),
            handled: true,
            ..Record::default()
        })?;
        self.reset_candidate();
        Ok(())
    }
    pub fn edit(&mut self, text: &str, now: i64) -> Result<(), String> {
        self.roll_day(now)?;
        let (hour, minute) = text.trim().split_once(':').ok_or("请输入 HH:MM")?;
        let hour: u32 = hour.parse().map_err(|_| "请输入 HH:MM")?;
        let minute: u32 = minute.parse().map_err(|_| "请输入 HH:MM")?;
        if hour > 23 || minute > 59 {
            return Err("时间超出范围".into());
        }
        #[cfg(unix)]
        {
            let timestamp = now as libc::time_t;
            if timestamp as i128 != i128::from(now) {
                return Err("时间超出范围".into());
            }
            let mut date = std::mem::MaybeUninit::<libc::tm>::uninit();
            if unsafe { libc::localtime_r(&timestamp, date.as_mut_ptr()) }.is_null() {
                return Err("无法读取本地日期".into());
            }
            let mut date = unsafe { date.assume_init() };
            date.tm_hour = hour as i32;
            date.tm_min = minute as i32;
            date.tm_sec = 0;
            date.tm_isdst = -1;
            let start = unsafe { libc::mktime(&mut date) } as i64;
            let converted = local_time(start).ok_or("时间超出范围")?;
            if converted.day != self.record.day
                || converted.hour != hour
                || converted.minute != minute
            {
                return Err("该本地时间不存在".into());
            }
            if start > now {
                return Err("起始时间不能晚于现在".into());
            }
            self.store(Record {
                day: self.record.day.clone(),
                started: Some(start),
                handled: true,
                reminded: false,
            })?;
            self.reset_candidate();
            Ok(())
        }
        #[cfg(not(unix))]
        {
            Err("当前平台暂不支持本地时间修改".into())
        }
    }
    pub fn observe(
        &mut self,
        config: &ClockConfig,
        now: i64,
        tick: Instant,
        idle: Option<Duration>,
    ) -> Result<bool, String> {
        self.roll_day(now)?;
        if !config.auto_start || self.record.handled {
            self.reset_candidate();
            return Ok(false);
        }
        let Some(idle) = idle else {
            self.reset_candidate();
            return Ok(false);
        };
        if idle >= Duration::from_secs(60)
            || self
                .last_sample
                .is_some_and(|t| tick.saturating_duration_since(t) > Duration::from_secs(10))
        {
            self.reset_candidate();
        }
        let input = tick.checked_sub(idle).unwrap_or(tick);
        let fresh = idle <= Duration::from_secs(5)
            && self.last_input.is_none_or(|last| {
                input.saturating_duration_since(last) >= Duration::from_millis(250)
            });
        self.last_sample = Some(tick);
        self.last_input = Some(input);
        if fresh && self.candidate.is_none() {
            let start = now.saturating_sub(idle.as_secs() as i64);
            if local_time(start).is_some_and(|t| t.day == self.record.day) {
                self.candidate = Some(Candidate {
                    started: start,
                    since: input,
                });
            }
        }
        if let Some(candidate) = &self.candidate {
            // Unavailable samples, suspend gaps and wall-clock jumps cannot confirm work.
            let elapsed = tick.saturating_duration_since(candidate.since).as_secs();
            if (now.saturating_sub(candidate.started) - elapsed as i64).abs() > 10 {
                self.reset_candidate();
                return Ok(false);
            }
            if elapsed >= config.confirmation_seconds && fresh {
                self.store(Record {
                    day: self.record.day.clone(),
                    started: Some(candidate.started),
                    handled: true,
                    reminded: false,
                })?;
                self.reset_candidate();
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn poll(&mut self, config: &ClockConfig, now: i64) -> Result<Option<String>, String> {
        self.roll_day(now)?;
        if self.running()
            && self
                .record
                .started
                .is_some_and(|start| (now - start) as f64 >= config.work_reminder_hours * 3600.)
        {
            let mut record = self.record.clone();
            record.reminded = true;
            self.store(record)?;
            Ok(Some(config.clock_out_message.clone()))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const BASE: i64 = 1_800_000_000;

    #[test]
    fn activity_confirms_backdated_start_once_and_survives_restart_and_reminder() {
        let root = std::env::temp_dir().join(format!("lexington-clock-{}", rand::random::<u64>()));
        let path = root.join("clock.json");
        let config = ClockConfig {
            work_reminder_hours: 1.,
            ..ClockConfig::default()
        };
        let tick = Instant::now();
        let mut clock = ClockState::load(path.clone(), BASE).unwrap();
        for seconds in (0..=180).step_by(2) {
            assert_eq!(
                clock
                    .observe(
                        &config,
                        BASE + seconds,
                        tick + Duration::from_secs(seconds as u64),
                        Some(Duration::ZERO)
                    )
                    .unwrap(),
                seconds == 180
            );
        }
        assert_eq!(clock.started(), Some(BASE));
        let mut clock = ClockState::load(path.clone(), BASE + 200).unwrap();
        assert!(clock.running());
        assert!(
            !clock
                .observe(
                    &config,
                    BASE + 200,
                    tick + Duration::from_secs(200),
                    Some(Duration::ZERO)
                )
                .unwrap()
        );
        assert_eq!(clock.started(), Some(BASE));
        assert!(clock.poll(&config, BASE + 3600).unwrap().is_some());
        assert_eq!(clock.started(), Some(BASE));
        assert!(!clock.running());
        let mut clock = ClockState::load(path.clone(), BASE + 3601).unwrap();
        assert!(clock.poll(&config, BASE + 3602).unwrap().is_none());
        clock.undo(BASE + 3603).unwrap();
        let mut clock = ClockState::load(path, BASE + 3604).unwrap();
        for seconds in (3604..=3800).step_by(2) {
            assert!(
                !clock
                    .observe(
                        &config,
                        BASE + seconds,
                        tick + Duration::from_secs(seconds as u64),
                        Some(Duration::ZERO)
                    )
                    .unwrap()
            );
        }
        assert!(clock.started().is_none());
        clock.toggle(&config, BASE + 3801).unwrap();
        assert_eq!(clock.started(), Some(BASE + 3801));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn idle_lock_unavailable_suspend_and_disabled_never_confirm_a_short_visit() {
        let config = ClockConfig::default();
        let tick = Instant::now();
        for interrupted in [None, Some(Duration::from_secs(60))] {
            let mut clock = ClockState::default();
            for seconds in (0..=100).step_by(2) {
                clock
                    .observe(
                        &config,
                        BASE + seconds,
                        tick + Duration::from_secs(seconds as u64),
                        Some(Duration::ZERO),
                    )
                    .unwrap();
            }
            clock
                .observe(
                    &config,
                    BASE + 102,
                    tick + Duration::from_secs(102),
                    interrupted,
                )
                .unwrap();
            for seconds in (104..=280).step_by(2) {
                assert!(
                    !clock
                        .observe(
                            &config,
                            BASE + seconds,
                            tick + Duration::from_secs(seconds as u64),
                            Some(Duration::ZERO)
                        )
                        .unwrap()
                );
            }
            assert!(clock.started().is_none());
        }
        let mut clock = ClockState::default();
        clock
            .observe(&config, BASE, tick, Some(Duration::ZERO))
            .unwrap();
        assert!(
            !clock
                .observe(
                    &config,
                    BASE + 200,
                    tick + Duration::from_secs(200),
                    Some(Duration::ZERO)
                )
                .unwrap()
        );
        assert!(clock.started().is_none());
        let mut clock = ClockState::default();
        for seconds in (0..=200).step_by(2) {
            assert!(
                !clock
                    .observe(
                        &config,
                        BASE + seconds,
                        tick + Duration::from_secs(seconds as u64),
                        Some(Duration::from_secs(seconds as u64))
                    )
                    .unwrap()
            );
        }
        let disabled = ClockConfig {
            auto_start: false,
            ..config
        };
        for seconds in (202..=400).step_by(2) {
            assert!(
                !clock
                    .observe(
                        &disabled,
                        BASE + seconds,
                        tick + Duration::from_secs(seconds as u64),
                        Some(Duration::ZERO)
                    )
                    .unwrap()
            );
        }
        assert!(clock.started().is_none());
    }

    #[test]
    fn clock_changes_restart_confirmation_and_next_day_can_start_again() {
        let config = ClockConfig::default();
        let tick = Instant::now();
        let mut clock = ClockState::default();
        for seconds in (0..=100).step_by(2) {
            clock
                .observe(
                    &config,
                    BASE + seconds,
                    tick + Duration::from_secs(seconds as u64),
                    Some(Duration::ZERO),
                )
                .unwrap();
        }
        assert!(
            !clock
                .observe(
                    &config,
                    BASE + 5000,
                    tick + Duration::from_secs(102),
                    Some(Duration::ZERO)
                )
                .unwrap()
        );
        assert!(clock.candidate.is_none());
        clock.undo(BASE).unwrap();
        for seconds in (0..=180).step_by(2) {
            assert_eq!(
                clock
                    .observe(
                        &config,
                        BASE + 86400 + seconds,
                        tick + Duration::from_secs(seconds as u64),
                        Some(Duration::ZERO)
                    )
                    .unwrap(),
                seconds == 180
            );
        }
        assert_eq!(clock.started(), Some(BASE + 86400));
    }

    #[test]
    fn editing_validates_local_time_and_disk_failures_do_not_commit_memory() {
        let root =
            std::env::temp_dir().join(format!("lexington-clock-edit-{}", rand::random::<u64>()));
        let path = root.join("clock.json");
        let mut clock = ClockState::load(path.clone(), BASE).unwrap();
        let time = local_time(BASE - 3600).unwrap().label();
        clock.edit(&time, BASE).unwrap();
        let original = clock.started();
        assert_eq!(local_time(original.unwrap()).unwrap().label(), time);
        for invalid in ["25:00", "12:60", "bad", "12:00:00"] {
            assert!(clock.edit(invalid, BASE).is_err());
            assert_eq!(clock.started(), original);
        }
        let future = local_time(BASE + 60).unwrap().label();
        assert!(clock.edit(&future, BASE).is_err());
        let loaded = ClockState::load(path.clone(), BASE).unwrap();
        assert_eq!(loaded.started(), original);
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(clock.undo(BASE).is_err());
        assert_eq!(clock.started(), original);
        fs::remove_dir(&path).unwrap();
        fs::write(&path, b"invalid json").unwrap();
        assert!(ClockState::load(path.clone(), BASE).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"invalid json");
        fs::remove_dir_all(root).unwrap();
    }
}
