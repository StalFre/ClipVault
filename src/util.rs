use std::process::Command;

#[cfg(windows)]
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new(program);
    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    c
}

#[cfg(not(windows))]
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    Command::new(program)
}

/// Mata el proceso y todos sus hijos (yt-dlp.exe lanza un proceso hijo y ffmpeg).
pub fn kill_tree(pid: u32) {
    let _ = command("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).output();
}

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Nombre de archivo seguro para Windows.
pub fn sanitize_filename(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 32 => '_',
            c => c,
        })
        .collect();
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut s: String = collapsed.trim_matches(|c| c == '.' || c == ' ').chars().take(120).collect();
    s = s.trim_end_matches(|c| c == '.' || c == ' ').to_string();
    if s.is_empty() {
        return "video".into();
    }
    let stem = s.split('.').next().unwrap_or("").to_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        s.insert(0, '_');
    }
    s
}

const PERMANENT: [&str; 11] = [
    "private video", "video unavailable", "this video is unavailable", "has been removed",
    "account associated with this video has been terminated", "copyright",
    "not available in your country", "members-only", "this live stream recording is not available",
    "sign in to confirm your age", "requested format is not available",
];

pub fn is_permanent_error(err: &str) -> bool {
    let e = err.to_lowercase();
    PERMANENT.iter().any(|m| e.contains(m))
}

pub fn fmt_duration(secs: f64) -> String {
    let t = secs.max(0.0) as u64;
    let (h, m, s) = (t / 3600, (t % 3600) / 60, t % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

#[cfg(windows)]
pub fn free_space(path: &std::path::Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut free: u64 = 0;
    let (mut total, mut total_free) = (0u64, 0u64);
    // SAFETY: punteros validos a variables locales; `wide` termina en NUL.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, &mut total_free) };
    (ok != 0).then_some(free)
}

#[cfg(not(windows))]
pub fn free_space(_: &std::path::Path) -> Option<u64> {
    None
}

pub fn now_stamp() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_basic() {
        assert_eq!(sanitize_filename("a:b/c\\d*e?f"), "a_b_c_d_e_f");
        assert_eq!(sanitize_filename("  ...  "), "video");
        assert_eq!(sanitize_filename("CON"), "_CON");
        assert_eq!(sanitize_filename("hola.  "), "hola");
    }

    #[test]
    fn permanent() {
        assert!(is_permanent_error("ERROR: Private video. Sign in"));
        assert!(!is_permanent_error("HTTP Error 503"));
    }
}
