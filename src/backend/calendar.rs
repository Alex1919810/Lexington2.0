use super::clock::local_time;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf};
use tyme4rs::tyme::{
    Culture,
    festival::{LUNAR_FESTIVAL_NAMES, LunarFestival, SOLAR_FESTIVAL_NAMES, SolarFestival},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('-');
        let date = Self {
            year: parts.next()?.parse().ok()?,
            month: parts.next()?.parse().ok()?,
            day: parts.next()?.parse().ok()?,
        };
        ((1900..=9999).contains(&date.year)
            && (1..=12).contains(&date.month)
            && (1..=date.days_in_month()).contains(&date.day)
            && parts.next().is_none()
            && date.label() == text)
            .then_some(date)
    }

    pub fn at(timestamp: i64) -> Option<Self> {
        Self::parse(&local_time(timestamp)?.day)
    }

    pub fn label(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub fn days_in_month(self) -> u32 {
        match self.month {
            4 | 6 | 9 | 11 => 30,
            2 if self.year % 4 == 0 && (self.year % 100 != 0 || self.year % 400 == 0) => 29,
            2 => 28,
            _ => 31,
        }
    }

    pub fn shift_month(self, offset: i32) -> Self {
        let index =
            (self.year * 12 + self.month as i32 - 1 + offset).clamp(1900 * 12, 9999 * 12 + 11);
        Self {
            year: index / 12,
            month: (index % 12 + 1) as u32,
            day: 1,
        }
    }

    /// Monday = 0. Gregorian civil arithmetic, independent of DST and timezone.
    pub fn first_weekday(self) -> u32 {
        let year = self.year - i32::from(self.month <= 2);
        let offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        ((year + year / 4 - year / 100 + year / 400 + offsets[self.month as usize - 1]) % 7) as u32
    }
}

/// Offline festival names, independent of overtime records and official days off.
/// Traditional festivals are supported for Gregorian years 1900 through 2100.
pub fn month_festivals(month: Date) -> BTreeMap<u32, Vec<String>> {
    let mut days: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for index in 0..SOLAR_FESTIVAL_NAMES.len() {
        if let Some(festival) = SolarFestival::from_index(month.year as isize, index) {
            let date = festival.get_day();
            if date.get_month() == month.month as usize {
                days.entry(date.get_day() as u32)
                    .or_default()
                    .push(festival.get_name());
            }
        }
    }
    if (1900..=2100).contains(&month.year) {
        // January can still belong to the previous lunar year. Enumerating both
        // also preserves festivals that coincide with a solar or term festival.
        for year in month.year - 1..=month.year {
            for index in 0..LUNAR_FESTIVAL_NAMES.len() {
                if let Some(festival) = LunarFestival::from_index(year as isize, index) {
                    let date = festival.get_day().get_solar_day();
                    if date.get_year() == month.year as isize
                        && date.get_month() == month.month as usize
                    {
                        let names = days.entry(date.get_day() as u32).or_default();
                        let name = festival.get_name();
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
            }
        }
    }
    for (m, day, name) in [
        (2, 14, "情人节"),
        (4, 1, "愚人节"),
        (12, 24, "平安夜"),
        (12, 25, "圣诞节"),
    ] {
        if month.month == m {
            days.entry(day).or_default().push(name.into());
        }
    }
    for (m, week, weekday, name) in [
        (5, 2, 6, "母亲节"),
        (6, 3, 6, "父亲节"),
        (11, 4, 3, "感恩节"),
    ] {
        if month.month == m {
            let day = 1 + (weekday + 7 - month.first_weekday()) % 7 + (week - 1) * 7;
            days.entry(day).or_default().push(name.into());
        }
    }
    days
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged, rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum Overtime {
    Measured { off_at: i64, checked_at: i64 },
    Manual { manual_minutes: u64 },
}

impl Overtime {
    pub fn minutes(&self) -> u64 {
        match self {
            Self::Measured { off_at, checked_at } => ((checked_at - off_at) / 60) as u64,
            Self::Manual { manual_minutes } => *manual_minutes,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    version: u32,
    days: BTreeMap<String, Overtime>,
}

pub struct CalendarState {
    path: PathBuf,
    record: Record,
}

impl CalendarState {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        let record: Record = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "加班记录损坏或格式不兼容，未覆盖原文件。")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Record {
                version: 1,
                days: BTreeMap::new(),
            },
            Err(error) => return Err(format!("读取加班记录失败：{error}")),
        };
        if record.version != 1
            || record.days.iter().any(|(day, item)| {
                Date::parse(day).is_none()
                    || match item {
                        Overtime::Measured { off_at, checked_at } => {
                            local_time(*off_at).is_none_or(|time| time.day != *day)
                                || Date::at(*checked_at).is_none()
                                || checked_at.checked_sub(*off_at).is_none_or(|s| s < 0)
                        }
                        Overtime::Manual { manual_minutes } => *manual_minutes > 24 * 60,
                    }
            })
        {
            return Err("加班记录无效或版本不兼容，未覆盖原文件。".into());
        }
        Ok(Self { path, record })
    }

    pub fn entry(&self, day: Date) -> Option<&Overtime> {
        self.record.days.get(&day.label())
    }

    pub fn month_total(&self, month: Date) -> (usize, u64) {
        let prefix = format!("{:04}-{:02}-", month.year, month.month);
        self.record
            .days
            .iter()
            .filter(|(day, item)| day.starts_with(&prefix) && item.minutes() > 0)
            .fold((0, 0), |(days, minutes), (_, item)| {
                (days + 1, minutes + item.minutes())
            })
    }

    pub fn check_in(&mut self, off_at: i64, now: i64) -> Result<Date, String> {
        let day = Date::at(off_at).ok_or("无法读取下班日期。")?;
        if Date::at(now).is_none() || now.checked_sub(off_at).is_none_or(|s| s < 0) {
            return Err("还未到下班时间，暂不能记录加班。".into());
        }
        let mut record = self.record.clone();
        record.days.insert(
            day.label(),
            Overtime::Measured {
                off_at,
                checked_at: now,
            },
        );
        self.store(record)?;
        Ok(day)
    }

    pub fn set_manual(&mut self, day: Date, hours: u32, minutes: u32) -> Result<(), String> {
        if Date::parse(&day.label()).is_none() {
            return Err("加班日期无效。".into());
        }
        if hours > 24 || minutes > 59 || (hours == 24 && minutes != 0) {
            return Err("每日加班时长需在 0～24 小时内，分钟需在 0～59 之间。".into());
        }
        let mut record = self.record.clone();
        record.days.insert(
            day.label(),
            Overtime::Manual {
                manual_minutes: u64::from(hours) * 60 + u64::from(minutes),
            },
        );
        self.store(record)
    }

    pub fn delete(&mut self, day: Date) -> Result<(), String> {
        let mut record = self.record.clone();
        record.days.remove(&day.label());
        self.store(record)
    }

    fn store(&mut self, record: Record) -> Result<(), String> {
        let parent = self.path.parent().ok_or("加班记录路径无效。")?;
        fs::create_dir_all(parent).map_err(|error| format!("保存加班记录失败：{error}"))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|error| format!("保存加班记录失败：{error}"))?;
        let bytes = serde_json::to_vec(&record).map_err(|_| "无法编码加班记录。")?;
        temporary
            .write_all(&bytes)
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|error| format!("保存加班记录失败：{error}"))?;
        temporary
            .persist(&self.path)
            .map_err(|error| format!("保存加班记录失败：{}", error.error))?;
        self.record = record;
        Ok(())
    }
}

pub fn duration_label(minutes: u64) -> String {
    format!("{}小时{:02}分", minutes / 60, minutes % 60)
}

pub fn normal_off(start: Option<i64>, work_hours: f64, now: i64) -> Result<i64, String> {
    let start = start.ok_or("今天尚未上班打卡，请先在打卡设置中记录上班时间。")?;
    if Date::at(start).is_none() || Date::at(start) != Date::at(now) {
        return Err("今天尚未上班打卡，请先记录今天的上班时间。".into());
    }
    if !work_hours.is_finite() || !(0.0..=87600.0).contains(&work_hours) {
        return Err("工作时长配置无效。".into());
    }
    start
        .checked_add((work_hours * 3600.).ceil() as i64)
        .filter(|time| local_time(*time).is_some())
        .ok_or_else(|| "无法计算下班时间。".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timestamp(day: &str, hour: i32, minute: i32) -> i64 {
        let date = Date::parse(day).unwrap();
        let mut time = crate::tools::time::local(1_800_000_000).unwrap();
        time.tm_year = date.year - 1900;
        time.tm_mon = date.month as i32 - 1;
        time.tm_mday = date.day as i32;
        time.tm_hour = hour;
        time.tm_min = minute;
        time.tm_sec = 0;
        time.tm_isdst = -1;
        crate::tools::time::timestamp(&mut time)
    }

    #[test]
    fn festivals_follow_lunar_years_terms_weekdays_and_coinciding_dates() {
        for (date, name) in [
            ("2026-01-01", "元旦"),
            ("2026-02-16", "除夕"),
            ("2026-02-17", "春节"),
            ("2027-02-06", "春节"),
            ("2026-03-03", "元宵节"),
            ("2026-04-05", "清明节"),
            ("2024-04-04", "清明节"),
            ("2026-05-10", "母亲节"),
            ("2026-06-19", "端午节"),
            ("2026-06-21", "父亲节"),
            ("2026-09-25", "中秋节"),
            ("2026-11-26", "感恩节"),
            ("2026-12-25", "圣诞节"),
            // The eve of a 29-day lunar December, in the previous lunar year.
            ("2025-01-28", "除夕"),
            ("2025-01-29", "春节"),
        ] {
            let day = Date::parse(date).unwrap();
            assert!(
                month_festivals(day)
                    .get(&day.day)
                    .is_some_and(|names| names.iter().any(|n| n == name)),
                "missing {name} on {date}"
            );
        }
        let october = month_festivals(Date::parse("2020-10-01").unwrap());
        assert_eq!(october[&1], ["国庆节", "中秋节"]);
        // Leap lunar May must not introduce a second Dragon Boat Festival.
        let june = month_festivals(Date::parse("2009-06-01").unwrap());
        assert!(!june.values().flatten().any(|name| name == "端午节"));
        assert!(!month_festivals(Date::parse("2026-11-12").unwrap()).contains_key(&12));
        for date in ["1900-01-01", "2100-12-01", "2101-01-01", "9999-01-01"] {
            let day = Date::parse(date).unwrap();
            let festivals = month_festivals(day);
            assert!(
                festivals
                    .keys()
                    .all(|n| (1..=day.days_in_month()).contains(n))
            );
            if day.month == 1 && day.year >= 1912 {
                assert!(festivals[&1].iter().any(|name| name == "元旦"));
            }
        }
    }

    #[test]
    fn calendar_grid_handles_leap_years_weekdays_and_year_boundaries() {
        assert!(Date::parse("2024-02-29").is_some());
        for invalid in [
            "2025-02-29",
            "1900-02-29",
            "2026-13-01",
            "2026-11-31",
            "2026-1-01",
        ] {
            assert!(Date::parse(invalid).is_none());
        }
        assert_eq!(Date::parse("2026-11-01").unwrap().first_weekday(), 6);
        assert_eq!(Date::parse("2024-01-01").unwrap().first_weekday(), 0);
        let december = Date::parse("2026-12-12").unwrap();
        assert_eq!(december.shift_month(1).label(), "2027-01-01");
        assert_eq!(
            december.shift_month(1).shift_month(-1).label(),
            "2026-12-01"
        );
    }

    #[test]
    fn overtime_updates_once_per_day_survives_restart_and_totals_by_month() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("calendar.json");
        let mut state = CalendarState::load(path.clone()).unwrap();
        let off = timestamp("2026-11-12", 18, 0);
        assert!(state.check_in(off, off - 1).is_err());
        let day = state.check_in(off, off + 3600).unwrap();
        state.check_in(off, off + 9030).unwrap();
        assert_eq!(state.entry(day).unwrap().minutes(), 150);
        let other = timestamp("2026-11-13", 18, 0);
        state.check_in(other, other + 3600).unwrap();
        // A late check-in belongs to the date on which normal work ended.
        let end_month = timestamp("2026-11-30", 23, 0);
        state.check_in(end_month, end_month + 7200).unwrap();
        let december = timestamp("2026-12-01", 18, 0);
        state.check_in(december, december + 3600).unwrap();
        let mut state = CalendarState::load(path.clone()).unwrap();
        assert_eq!(state.month_total(day), (3, 330));
        assert_eq!(state.month_total(Date::at(december).unwrap()), (1, 60));
        state.delete(day).unwrap();
        assert_eq!(
            CalendarState::load(path.clone()).unwrap().month_total(day),
            (2, 180)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn normal_off_uses_today_start_and_configured_work_hours() {
        let start = timestamp("2026-11-12", 9, 0);
        assert_eq!(
            normal_off(Some(start), 8.5, start + 36000).unwrap(),
            start + 30600
        );
        assert!(normal_off(None, 8., start).is_err());
        assert!(normal_off(Some(start), 8., timestamp("2026-11-13", 9, 0)).is_err());
        assert!(normal_off(Some(start), f64::NAN, start).is_err());
    }

    #[test]
    fn manual_overtime_preserves_legacy_records_and_updates_month_totals() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("calendar.json");
        let off = timestamp("2026-11-12", 18, 0);
        // The pre-manual schema remains readable without rewriting the file.
        let legacy = serde_json::json!({"version":1,"days":{"2026-11-12":{"offAt":off,"checkedAt":off+3600}}});
        fs::write(&path, legacy.to_string()).unwrap();
        let mut state = CalendarState::load(path.clone()).unwrap();
        let day = Date::parse("2026-11-10").unwrap();
        state.set_manual(day, 2, 30).unwrap();
        assert_eq!(state.month_total(day), (2, 210));
        state.set_manual(day, 3, 15).unwrap();
        let mut state = CalendarState::load(path.clone()).unwrap();
        assert_eq!(state.month_total(day), (2, 255));
        assert!(matches!(
            state.entry(day),
            Some(Overtime::Manual {
                manual_minutes: 195
            })
        ));
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["days"]["2026-11-12"], legacy["days"]["2026-11-12"]);
        assert_eq!(
            saved["days"]["2026-11-10"],
            serde_json::json!({"manualMinutes":195})
        );
        for (hours, minutes) in [(25, 0), (24, 1), (1, 60), (u32::MAX, 0)] {
            assert!(state.set_manual(day, hours, minutes).is_err());
        }
        assert_eq!(state.entry(day).unwrap().minutes(), 195);
        state.set_manual(day, 24, 0).unwrap();
        state.set_manual(day, 0, 0).unwrap();
        assert_eq!(state.month_total(day), (1, 60));
        // Manual correction replaces an automatic check-in without inventing times.
        state.set_manual(Date::at(off).unwrap(), 2, 0).unwrap();
        assert_eq!(state.month_total(day), (1, 120));
        state.check_in(off, off + 3600).unwrap();
        assert_eq!(state.month_total(day), (1, 60));
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(state.set_manual(day, 4, 0).is_err());
        assert_eq!(state.entry(day).unwrap().minutes(), 0);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn invalid_calendar_and_write_failures_preserve_records() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("calendar.json");
        for invalid in [
            "broken",
            r#"{"version":2,"days":{}}"#,
            r#"{"version":1,"days":{"2026-02-30":{"offAt":0,"checkedAt":1}}}"#,
            r#"{"version":1,"days":{"2026-11-12":{"manualMinutes":1441}}}"#,
            r#"{"version":1,"days":{"2026-11-12":{"manualMinutes":-1}}}"#,
            r#"{"version":1,"days":{"2026-11-12":{"manualMinutes":60,"offAt":0,"checkedAt":1}}}"#,
        ] {
            fs::write(&path, invalid).unwrap();
            assert!(CalendarState::load(path.clone()).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        }
        fs::remove_file(&path).unwrap();
        let mut state = CalendarState::load(path.clone()).unwrap();
        let off = timestamp("2026-11-12", 18, 0);
        let day = state.check_in(off, off + 3600).unwrap();
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(state.check_in(off, off + 7200).is_err());
        assert!(state.delete(day).is_err());
        assert_eq!(state.entry(day).unwrap().minutes(), 60);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
