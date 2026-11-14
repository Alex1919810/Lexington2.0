use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DialogConfig {
    pub width: f32,
    pub max_height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub auto_close_delay: f64,
}
impl Default for DialogConfig {
    fn default() -> Self {
        Self {
            width: 200.,
            max_height: 120.,
            offset_x: 100.,
            offset_y: 50.,
            auto_close_delay: 10.,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodexConfig {
    // Runtime launch options only. Legacy connection fields in config.json are ignored.
    #[serde(skip)]
    pub executable: String,
    #[serde(skip)]
    pub working_directory: Option<PathBuf>,
    #[serde(skip)]
    pub connection_revision: u64,
    pub no_approval: bool,
    pub development_mode: bool,
}
impl CodexConfig {
    pub fn connection_changed(&self, other: &Self) -> bool {
        self.executable != other.executable
            || self.working_directory != other.working_directory
            || self.connection_revision != other.connection_revision
            || self.development_mode != other.development_mode
    }
}
impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            executable: "codex".into(),
            working_directory: None,
            connection_revision: 0,
            no_approval: false,
            development_mode: false,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClockConfig {
    pub auto_start: bool,
    pub confirmation_seconds: u64,
    pub work_reminder_hours: f64,
    pub clock_in_message: String,
    pub clock_out_message: String,
    pub clock_in_cancel_message: String,
}
impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            auto_start: true,
            confirmation_seconds: 180,
            work_reminder_hours: 12.,
            clock_in_message: "打卡成功！".into(),
            clock_out_message: "下班时间到了！".into(),
            clock_in_cancel_message: "打卡已终止".into(),
        }
    }
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WeatherConfig {
    pub city: Option<super::weather::City>,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub dialog: DialogConfig,
    pub codex: CodexConfig,
    pub clock_in: ClockConfig,
    pub weather: WeatherConfig,
}
impl Config {
    pub fn save_weather(root: &Path, city: Option<&super::weather::City>) -> Result<(), String> {
        if let Some(city) = city {
            city.validate()?;
        }
        let value = serde_json::to_value(city).map_err(|_| "城市配置无法保存。".to_owned())?;
        Self::save_settings(root, "weather", [("city", value)])
    }

    pub fn save_no_approval(root: &Path, enabled: bool) -> Result<(), String> {
        Self::save_settings(root, "codex", [("noApproval", enabled.into())])
    }
    pub fn save_auto_clock(root: &Path, enabled: bool) -> Result<(), String> {
        Self::save_settings(root, "clockIn", [("autoStart", enabled.into())])
    }
    pub fn save_development_mode(root: &Path, enabled: bool) -> Result<(), String> {
        Self::save_settings(root, "codex", [("developmentMode", enabled.into())])
    }
    fn save_settings(
        root: &Path,
        section: &str,
        values: impl IntoIterator<Item = (&'static str, serde_json::Value)>,
    ) -> Result<(), String> {
        use std::io::Write;

        let path = root.join("config/config.json");
        let mut document: serde_json::Value = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "配置格式错误，未保存设置。".to_owned())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
            Err(e) => return Err(format!("读取配置失败：{e}")),
        };
        let group = document
            .as_object_mut()
            .ok_or("配置必须为 JSON 对象，未保存设置。")?
            .entry(section)
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .ok_or("设置分组必须为对象，未保存设置。")?;
        for (key, value) in values {
            group.insert(key.into(), value);
        }
        let mut contents = serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?;
        contents.push(b'\n');
        fs::create_dir_all(root.join("config")).map_err(|e| format!("创建配置目录失败：{e}"))?;
        let temporary = path.with_extension(format!("{}.tmp", rand::random::<u64>()));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|e| format!("保存配置失败：{e}"))?;
        let written = file.write_all(&contents).and_then(|_| file.sync_all());
        drop(file);
        let result = written.and_then(|_| fs::rename(&temporary, &path));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|e| format!("保存配置失败：{e}"))
    }
    pub fn load(root: &Path) -> Self {
        Self::read(root).unwrap_or_default()
    }
    pub fn read(root: &Path) -> Result<Self, String> {
        let bytes =
            fs::read(root.join("config/config.json")).map_err(|e| format!("读取配置失败：{e}"))?;
        let mut config: Self = serde_json::from_slice(&bytes)
            .map_err(|_| "配置格式错误，保留上次有效设置。".to_owned())?;
        if let Some(city) = &config.weather.city {
            city.validate()?;
        }
        // Keep malformed sizes/timers from crashing the native renderer.
        config.dialog.width = config.dialog.width.clamp(40., 2000.);
        config.dialog.max_height = config.dialog.max_height.clamp(24., 2000.);
        config.dialog.auto_close_delay = config.dialog.auto_close_delay.clamp(0., 86400.);
        config.clock_in.work_reminder_hours = config.clock_in.work_reminder_hours.clamp(0., 87600.);
        config.clock_in.confirmation_seconds = config.clock_in.confirmation_seconds.clamp(30, 600);
        Ok(config)
    }
}
