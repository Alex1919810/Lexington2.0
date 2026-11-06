use std::path::PathBuf;

pub fn resource_root() -> PathBuf {
    if let Some(root) = std::env::var_os("LEXINGTON_HOME") {
        return PathBuf::from(root);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
        && dir.join("resource").is_dir()
    {
        return dir.into();
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
