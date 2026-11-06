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
#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodexConfig {
    pub executable: String,
    pub working_directory: Option<PathBuf>,
}
impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            executable: "codex".into(),
            working_directory: None,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClockConfig {
    pub work_reminder_hours: f64,
    pub clock_in_message: String,
    pub clock_out_message: String,
    pub clock_in_cancel_message: String,
}
impl Default for ClockConfig {
    fn default() -> Self {
        Self {
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
    pub fn load(root: &Path) -> Self {
        let mut config: Self = fs::read(root.join("config/config.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        // Keep malformed sizes/timers from crashing the native renderer.
        config.dialog.width = config.dialog.width.clamp(40., 2000.);
        config.dialog.max_height = config.dialog.max_height.clamp(24., 2000.);
        config.dialog.auto_close_delay = config.dialog.auto_close_delay.clamp(0., 86400.);
        config.clock_in.work_reminder_hours = config.clock_in.work_reminder_hours.clamp(0., 87600.);
        config
    }
}
