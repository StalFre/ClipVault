//! Autoactualizacion de ClipVault desde GitHub Releases.
use std::io::{Read, Write};
use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const API: &str = "https://api.github.com/repos/StalFre/ClipVault/releases/latest";

#[derive(Clone, Debug)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
    pub name: String,
}

/// "Admin" si la app vive en Archivos de programa; si no, "User".
pub fn variant() -> &'static str {
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
    let pf: Vec<String> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .map(|s| s.to_lowercase())
        .collect();
    if pf.iter().any(|p| exe.starts_with(p.as_str())) { "Admin" } else { "User" }
}

fn parse(v: &str) -> Vec<u32> {
    v.trim().trim_start_matches(['v', 'V']).split(['.', '-']).map_while(|p| p.parse().ok()).collect()
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    let (a, b) = (parse(candidate), parse(current));
    let n = a.len().max(b.len());
    for i in 0..n {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(60))
        .user_agent("ClipVault-Updater")
        .build()
}

/// Ok(None) = ya estas al dia.
pub fn check() -> Result<Option<UpdateInfo>, String> {
    let v: serde_json::Value = agent()
        .get(API)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().ok_or("respuesta sin tag_name")?.to_string();
    if !is_newer(&tag, VERSION) {
        return Ok(None);
    }
    let suffix = format!("-{}.exe", variant());
    let asset = v["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"].as_str().map_or(false, |n| n.starts_with("ClipVault-Setup") && n.ends_with(&suffix))))
        .ok_or("la versión nueva no trae instalador para esta instalación")?;
    Ok(Some(UpdateInfo {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        url: asset["browser_download_url"].as_str().ok_or("asset sin url")?.to_string(),
        name: asset["name"].as_str().unwrap_or("ClipVault-Setup.exe").to_string(),
    }))
}

/// Descarga el instalador y lo lanza en modo silencioso (pide UAC si es la version Admin).
pub fn download_and_run(info: &UpdateInfo) -> Result<(), String> {
    let dest = std::env::temp_dir().join(&info.name);
    let resp = agent().get(&info.url).call().map_err(|e| e.to_string())?;
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
    }
    drop(file);
    // `start` usa ShellExecute: muestra el aviso de administrador cuando hace falta.
    crate::util::command("cmd")
        .args(["/C", "start", "", &dest.to_string_lossy(), "/SILENT", "/SUPPRESSMSGBOXES", "/CLOSEAPPLICATIONS"])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("v2.0.1", "2.0.0"));
        assert!(is_newer("2.1", "2.0.9"));
        assert!(!is_newer("2.0.0", "2.0.0"));
        assert!(!is_newer("1.9.9", "2.0.0"));
    }
}
