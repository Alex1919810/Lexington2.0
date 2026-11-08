use std::path::PathBuf;

/// Called on a worker thread so the native picker does not block GUI updates.
pub fn pick_illustrations() -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new()
        .set_title("导入角色立绘")
        .add_filter("立绘（PNG / GIF）", &["png", "gif"])
        .pick_files()
}
