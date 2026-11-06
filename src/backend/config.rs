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
#[derive(Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodexConfig {
    pub executable: String,
    pub working_directory: Option<PathBuf>,
    pub no_approval: bool,
    pub model: String,
    pub use_custom_api: bool,
    pub base_url: String,
    pub api_key: String,
}
impl std::fmt::Debug for CodexConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexConfig")
            .field("executable", &self.executable)
            .field("working_directory", &self.working_directory)
            .field("no_approval", &self.no_approval)
            .field("model", &self.model)
            .field("use_custom_api", &self.use_custom_api)
            .field("base_url", &self.base_url)
            .field("api_key", &"[hidden]")
            .finish()
    }
}
impl CodexConfig {
    pub fn validate_connection(&self) -> Result<(), String> {
        if self.model.len() > 200 || self.model.chars().any(char::is_control) {
            return Err("模型名称无效。".into());
        }
        if !self.use_custom_api {
            return Ok(());
        }
        if self.model.trim().is_empty() {
            return Err("请输入模型名称。".into());
        }
        let valid = url::Url::parse(&self.base_url).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
        });
        if !valid {
            return Err("API 地址需为 http/https 基础地址，不含账号、查询参数或片段。".into());
        }
        if self.api_key.trim().is_empty()
            || self.api_key.len() > 8192
            || self.api_key.chars().any(char::is_control)
        {
            return Err("请输入有效的 API Key。".into());
        }
        Ok(())
    }
    pub fn connection_changed(&self, other: &Self) -> bool {
        self.executable != other.executable
            || self.working_directory != other.working_directory
            || self.model != other.model
            || self.use_custom_api != other.use_custom_api
            || (self.use_custom_api
                && (self.base_url != other.base_url || self.api_key != other.api_key))
    }
}
impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            executable: "codex".into(),
            working_directory: None,
            no_approval: false,
            model: String::new(),
            use_custom_api: false,
            base_url: String::new(),
            api_key: String::new(),
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
pub struct Config {
    pub dialog: DialogConfig,
    pub codex: CodexConfig,
    pub clock_in: ClockConfig,
}
impl Config {
    pub fn save_no_approval(root: &Path, enabled: bool) -> Result<(), String> {
        Self::save_settings(root, "codex", [("noApproval", enabled.into())])
    }
    pub fn save_auto_clock(root: &Path, enabled: bool) -> Result<(), String> {
        Self::save_settings(root, "clockIn", [("autoStart", enabled.into())])
    }
    pub fn save_connection(root: &Path, config: &CodexConfig) -> Result<(), String> {
        let mut config = config.clone();
        config.model = config.model.trim().to_owned();
        config.base_url = config.base_url.trim().trim_end_matches('/').to_owned();
        config.api_key = config.api_key.trim().to_owned();
        config.validate_connection()?;
        Self::save_settings(
            root,
            "codex",
            [
                ("model", config.model.clone().into()),
                ("useCustomApi", config.use_custom_api.into()),
                ("baseUrl", config.base_url.clone().into()),
                ("apiKey", config.api_key.clone().into()),
            ],
        )
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
        config.codex.model = config.codex.model.trim().to_owned();
        config.codex.base_url = config
            .codex
            .base_url
            .trim()
            .trim_end_matches('/')
            .to_owned();
        config.codex.api_key = config.codex.api_key.trim().to_owned();
        config.codex.validate_connection()?;
        // Keep malformed sizes/timers from crashing the native renderer.
        config.dialog.width = config.dialog.width.clamp(40., 2000.);
        config.dialog.max_height = config.dialog.max_height.clamp(24., 2000.);
        config.dialog.auto_close_delay = config.dialog.auto_close_delay.clamp(0., 86400.);
        config.clock_in.work_reminder_hours = config.clock_in.work_reminder_hours.clamp(0., 87600.);
        config.clock_in.confirmation_seconds = config.clock_in.confirmation_seconds.clamp(30, 600);
        Ok(config)
    }
}
