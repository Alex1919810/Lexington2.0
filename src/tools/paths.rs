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

pub fn clock_state_file(dev: bool) -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir);
    data.join("lexington")
        .join(if dev { "clock-dev.json" } else { "clock.json" })
}
