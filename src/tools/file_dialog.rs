use std::path::{Path, PathBuf};

/// Called on a worker thread so the native picker does not block GUI updates.
pub fn pick_illustrations() -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new()
        .set_title("导入角色立绘")
        .add_filter("立绘（PNG / GIF）", &["png", "gif"])
        .pick_files()
}

/// Returns only the chosen directory; cancellation leaves the current project unchanged.
pub fn pick_project_directory(initial: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title("选择 development 项目目录");
    if let Some(directory) = initial.filter(|path| path.is_dir()) {
        dialog = dialog.set_directory(directory);
        // rfd's Zenity fallback reads file_name, while Portal uses starting_directory.
        #[cfg(target_os = "linux")]
        if let Some(directory) = directory.to_str() {
            dialog = dialog.set_file_name(format!("{}/", directory.trim_end_matches('/')));
        }
    }
    dialog.pick_folder()
}
