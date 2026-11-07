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
    let data = data_root();
    data.join(if dev { "clock-dev.json" } else { "clock.json" })
}

pub fn data_root() -> PathBuf {
    #[cfg(windows)]
    let data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join("AppData/Local"))
        })
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(not(windows))]
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir);
    data.join("lexington")
}

pub fn alarm_state_file(dev: bool) -> PathBuf {
    clock_state_file(dev).with_file_name(if dev {
        "alarms-dev.json"
    } else {
        "alarms.json"
    })
}
