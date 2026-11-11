use super::clock::local_time;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Alarm {
    pub id: u64,
    pub time: String,
    pub message: String,
    pub daily: bool,
    pub enabled: bool,
    pub next_due: i64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reminder {
    pub time: String,
    pub message: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    alarms: Vec<Alarm>,
    pending: Vec<Reminder>,
}
#[derive(Default)]
pub struct AlarmState {
    record: Record,
    path: Option<PathBuf>,
}

/// Next occurrence of a local HH:MM, strictly after now. mktime handles local
/// calendar days rather than adding 86400 seconds across DST changes.
pub(super) fn next_due(text: &str, now: i64) -> Result<i64, String> {
    let bytes = text.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || !bytes[..2].iter().chain(&bytes[3..]).all(u8::is_ascii_digit)
    {
        return Err("请输入 HH:MM".into());
    }
    let hour: i32 = text[..2].parse().map_err(|_| "请输入 HH:MM")?;
    let minute: i32 = text[3..].parse().map_err(|_| "请输入 HH:MM")?;
    if hour > 23 || minute > 59 {
        return Err("时间超出范围".into());
    }
    {
        let date = crate::tools::time::local(now).ok_or("无法读取本地日期")?;
        // A nonexistent spring-forward time is skipped, not shifted an hour.
        for offset in 0..=3 {
            let mut target = date;
            target.tm_mday += offset;
            target.tm_hour = hour;
            target.tm_min = minute;
            target.tm_sec = 0;
            target.tm_isdst = -1;
            let result = crate::tools::time::timestamp(&mut target);
            if result > now
                && local_time(result)
                    .is_some_and(|t| t.hour == hour as u32 && t.minute == minute as u32)
            {
                return Ok(result);
            }
        }
        Err("无法计算下次闹钟时间".into())
    }
}

impl AlarmState {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        let record: Record = match fs::read(&path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|_| "闹钟记录损坏，未覆盖原文件。")?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Record::default(),
            Err(e) => return Err(format!("读取闹钟失败：{e}")),
        };
        let mut ids = std::collections::HashSet::new();
        if record.alarms.iter().any(|a| {
            !ids.insert(a.id)
                || a.message.trim().is_empty()
                || a.message.chars().count() > 500
                || next_due(&a.time, a.next_due).is_err()
                || local_time(a.next_due).is_none_or(|t| t.label() != a.time)
        }) || record.pending.iter().any(|r| {
            r.message.trim().is_empty()
                || r.message.chars().count() > 500
                || next_due(&r.time, 1_800_000_000).is_err()
        }) {
            return Err("闹钟记录无效，未覆盖原文件。".into());
        }
        Ok(Self {
            record,
            path: Some(path),
        })
    }
    fn store(&mut self, record: Record) -> Result<(), String> {
        if let Some(path) = &self.path {
            use std::io::Write;
            let parent = path.parent().ok_or("闹钟路径无效")?;
            fs::create_dir_all(parent).map_err(|e| format!("保存闹钟失败：{e}"))?;
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
                .map_err(|e| format!("保存闹钟失败：{e}"))?;
            let bytes = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
            let result = file.write_all(&bytes).and_then(|_| file.sync_all());
            drop(file);
            if let Err(e) = result.and_then(|_| fs::rename(&temporary, path)) {
                let _ = fs::remove_file(temporary);
                return Err(format!("保存闹钟失败：{e}"));
            }
        }
        self.record = record;
        Ok(())
    }
    pub fn alarms(&self) -> &[Alarm] {
        &self.record.alarms
    }
    pub fn pending(&self) -> &[Reminder] {
        &self.record.pending
    }
    pub fn save(
        &mut self,
        id: Option<u64>,
        time: &str,
        message: &str,
        daily: bool,
        now: i64,
    ) -> Result<(), String> {
        let message = message.trim();
        if message.is_empty() || message.chars().count() > 500 {
            return Err("提示语需为 1～500 个字符".into());
        }
        let due = next_due(time, now)?;
        let mut record = self.record.clone();
        if let Some(id) = id {
            let alarm = record
                .alarms
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or("闹钟已不存在")?;
            *alarm = Alarm {
                id,
                time: time.into(),
                message: message.into(),
                daily,
                enabled: alarm.enabled,
                next_due: due,
            };
        } else {
            let id = loop {
                let id = rand::random();
                if record.alarms.iter().all(|a| a.id != id) {
                    break id;
                }
            };
            record.alarms.push(Alarm {
                id,
                time: time.into(),
                message: message.into(),
                daily,
                enabled: true,
                next_due: due,
            });
        }
        self.store(record)
    }
    pub fn toggle(&mut self, id: u64, now: i64) -> Result<(), String> {
        let mut record = self.record.clone();
        let alarm = record
            .alarms
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or("闹钟已不存在")?;
        alarm.enabled = !alarm.enabled;
        if alarm.enabled {
            alarm.next_due = next_due(&alarm.time, now)?;
        }
        self.store(record)
    }
    pub fn delete(&mut self, id: u64) -> Result<(), String> {
        let mut record = self.record.clone();
        record.alarms.retain(|a| a.id != id);
        self.store(record)
    }
    pub fn poll(&mut self, now: i64) -> Result<(), String> {
        if !self
            .record
            .alarms
            .iter()
            .any(|a| a.enabled && a.next_due <= now)
        {
            return Ok(());
        }
        let mut record = self.record.clone();
        for alarm in &mut record.alarms {
            if alarm.enabled && alarm.next_due <= now {
                record.pending.push(Reminder {
                    time: alarm.time.clone(),
                    message: alarm.message.clone(),
                });
                if alarm.daily {
                    alarm.next_due = next_due(&alarm.time, now)?;
                } else {
                    alarm.enabled = false;
                }
            }
        }
        self.store(record)
    }
    pub fn dismiss(&mut self) -> Result<(), String> {
        let mut record = self.record.clone();
        record.pending.clear();
        self.store(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: i64 = 1_800_000_000;
    #[test]
    fn alarms_fire_together_once_and_daily_reschedules_without_duplicate_on_restart() {
        let root = std::env::temp_dir().join(format!("lexington-alarm-{}", rand::random::<u64>()));
        let path = root.join("alarms.json");
        let time = local_time(NOW + 120).unwrap().label();
        let mut state = AlarmState::load(path.clone()).unwrap();
        state.save(None, &time, " 喝水 ", false, NOW).unwrap();
        state.save(None, &time, "站起来走走", true, NOW).unwrap();
        let due = state.alarms()[0].next_due;
        state.poll(due - 1).unwrap();
        assert!(state.pending().is_empty());
        state.poll(due).unwrap();
        assert_eq!(state.pending().len(), 2);
        assert_eq!(state.pending()[0].message, "喝水");
        assert!(!state.alarms()[0].enabled);
        let tomorrow = state.alarms()[1].next_due;
        assert!(tomorrow > due);
        assert_eq!(local_time(tomorrow).unwrap().label(), time);
        let mut state = AlarmState::load(path.clone()).unwrap();
        state.poll(due + 30).unwrap();
        assert_eq!(state.pending().len(), 2);
        state.dismiss().unwrap();
        assert!(AlarmState::load(path.clone()).unwrap().pending().is_empty());
        // Resume after multiple missed days: one reminder, next occurrence in future.
        state.poll(tomorrow + 3 * 86400 + 30).unwrap();
        assert_eq!(state.pending().len(), 1);
        assert!(state.alarms()[1].next_due > tomorrow + 3 * 86400 + 30);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn editing_enabling_validation_and_storage_failures_preserve_state() {
        let root =
            std::env::temp_dir().join(format!("lexington-alarm-edit-{}", rand::random::<u64>()));
        let path = root.join("alarms.json");
        let mut state = AlarmState::load(path.clone()).unwrap();
        for invalid in ["25:00", "12:60", "1:00", "bad", "１２:３０"] {
            assert!(state.save(None, invalid, "提醒", false, NOW).is_err());
        }
        assert!(state.save(None, "12:00", " ", false, NOW).is_err());
        assert!(
            state
                .save(None, "12:00", &"字".repeat(501), false, NOW)
                .is_err()
        );
        state.save(None, "12:00", "提醒", false, NOW).unwrap();
        let id = state.alarms()[0].id;
        state.toggle(id, NOW).unwrap();
        state.poll(NOW + 86400 * 2).unwrap();
        assert!(state.pending().is_empty());
        state.save(Some(id), "13:00", "新提示", true, NOW).unwrap();
        assert!(!state.alarms()[0].enabled);
        state.toggle(id, NOW + 86400 * 2).unwrap();
        assert!(state.alarms()[0].next_due > NOW + 86400 * 2);
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(state.delete(id).is_err());
        assert_eq!(state.alarms().len(), 1);
        assert!(state.poll(state.alarms()[0].next_due).is_err());
        assert!(state.pending().is_empty());
        assert!(state.alarms()[0].enabled);
        fs::remove_dir(&path).unwrap();
        state.delete(id).unwrap();
        assert!(AlarmState::load(path.clone()).unwrap().alarms().is_empty());
        fs::write(&path, b"invalid").unwrap();
        assert!(AlarmState::load(path.clone()).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"invalid");
        fs::remove_dir_all(root).unwrap();
    }
}
