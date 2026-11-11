use std::path::PathBuf;

fn ensure(p: PathBuf) -> PathBuf {
    let _ = std::fs::create_dir_all(&p);
    p
}

pub fn home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// %LOCALAPPDATA%\ClipVault: siempre escribible, sin admin.
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(home);
    ensure(base.join("ClipVault"))
}

pub fn bin_dir() -> PathBuf {
    ensure(data_dir().join("bin"))
}

pub fn log_dir() -> PathBuf {
    ensure(data_dir().join("logs"))
}

pub fn default_download_dir() -> PathBuf {
    ensure(home().join("Downloads"))
}

pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))
}
