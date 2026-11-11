//! Gestion de yt-dlp y ffmpeg: localizar, versionar y actualizar al iniciar.
use crate::{paths, util};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const YTDLP_API: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
const FFMPEG_ZIP: &str = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
const FFMPEG_VERSION_URL: &str = "https://www.gyan.dev/ffmpeg/builds/release-version";

#[derive(Clone, Default, Debug)]
pub struct ToolsInfo {
    pub ytdlp: String,
    pub ffmpeg: String,
    pub ytdlp_latest: String,
    pub ffmpeg_latest: String,
    pub status: String,
    pub busy: bool,
    /// 0..1, o negativo = indeterminado
    pub progress: f32,
}

pub fn ytdlp_path() -> PathBuf {
    let p = paths::bin_dir().join("yt-dlp.exe");
    if p.is_file() { p } else { PathBuf::from("yt-dlp") }
}

pub fn ffmpeg_dir() -> Option<PathBuf> {
    let d = paths::bin_dir();
    d.join("ffmpeg.exe").is_file().then_some(d)
}

pub fn ready() -> bool {
    paths::bin_dir().join("yt-dlp.exe").is_file() && ffmpeg_dir().is_some()
}

pub fn ytdlp_version() -> Option<String> {
    version_of(&ytdlp_path(), &["--version"]).map(|s| s.lines().next().unwrap_or("").trim().to_string())
}

pub fn ffmpeg_version() -> Option<String> {
    let exe = paths::bin_dir().join("ffmpeg.exe");
    if !exe.is_file() {
        return None;
    }
    let out = version_of(&exe, &["-version"])?;
    let first = out.lines().next()?;
    let ver = first.split_whitespace().nth(2)?;
    // "7.1.1-essentials_build-www.gyan.dev" -> "7.1.1"; builds "N-123-g..." se dejan completos.
    Some(if ver.starts_with("N-") { ver.to_string() } else { ver.split('-').next().unwrap_or(ver).to_string() })
}

fn version_of(exe: &Path, args: &[&str]) -> Option<String> {
    let out = util::command(exe).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .user_agent("ClipVault/2.0")
        .build()
}

fn set(info: &Mutex<ToolsInfo>, f: impl FnOnce(&mut ToolsInfo)) {
    if let Ok(mut g) = info.lock() {
        f(&mut g);
    }
}

/// Binarios que vengan junto al .exe o en la carpeta actual se copian a la carpeta de datos.
fn migrate_local_copies() {
    let bin = paths::bin_dir();
    let mut sources: Vec<PathBuf> = Vec::new();
    if let Some(d) = paths::exe_dir() {
        sources.push(d);
    }
    if let Ok(d) = std::env::current_dir() {
        sources.push(d);
    }
    for name in ["yt-dlp.exe", "ffmpeg.exe", "ffprobe.exe"] {
        let dest = bin.join(name);
        if dest.is_file() {
            continue;
        }
        if let Some(src) = sources.iter().map(|d| d.join(name)).find(|p| p.is_file()) {
            match std::fs::copy(&src, &dest) {
                Ok(_) => log::info!("{name} copiado desde {}", src.display()),
                Err(e) => log::warn!("No se pudo copiar {name}: {e}"),
            }
        }
    }
}

fn download_file(url: &str, dest: &Path, info: &Mutex<ToolsInfo>, label: &str) -> Result<(), String> {
    let resp = agent().get(url).call().map_err(|e| format!("{label}: {e}"))?;
    let total: Option<u64> = resp.header("Content-Length").and_then(|v| v.parse().ok());
    let mut reader = resp.into_reader();
    let mut file = File::create(dest).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 64 * 1024];
    let (mut done, mut last) = (0u64, Instant::now());
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("{label}: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if last.elapsed() > Duration::from_millis(200) {
            last = Instant::now();
            let frac = total.map(|t| (done as f32 / t as f32).min(1.0)).unwrap_or(-1.0);
            let mb = done as f32 / 1_048_576.0;
            set(info, |i| {
                i.progress = frac;
                i.status = if frac >= 0.0 {
                    format!("Descargando {label}… {:.0}%", frac * 100.0)
                } else {
                    format!("Descargando {label}… {mb:.1} MB")
                };
            });
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn replace_file(new: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        std::fs::remove_file(target).map_err(|e| format!("No se pudo reemplazar {}: {e}", target.display()))?;
    }
    std::fs::rename(new, target).map_err(|e| e.to_string())
}

fn latest_ytdlp() -> Result<String, String> {
    let v: serde_json::Value = agent()
        .get(YTDLP_API)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    v["tag_name"].as_str().map(|s| s.trim().to_string()).ok_or_else(|| "respuesta sin tag_name".into())
}

fn latest_ffmpeg() -> Result<String, String> {
    let s = agent().get(FFMPEG_VERSION_URL).call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())?;
    Ok(s.trim().to_string())
}

fn install_ytdlp(info: &Mutex<ToolsInfo>) -> Result<(), String> {
    let bin = paths::bin_dir();
    let new = bin.join("yt-dlp.exe.new");
    download_file(YTDLP_URL, &new, info, "yt-dlp")?;
    if version_of(&new, &["--version"]).is_none() {
        let _ = std::fs::remove_file(&new);
        return Err("el yt-dlp descargado no se ejecuta".into());
    }
    replace_file(&new, &bin.join("yt-dlp.exe"))
}

fn install_ffmpeg(info: &Mutex<ToolsInfo>) -> Result<(), String> {
    let bin = paths::bin_dir();
    let zip_path = bin.join("ffmpeg.zip.part");
    download_file(FFMPEG_ZIP, &zip_path, info, "ffmpeg")?;
    set(info, |i| {
        i.status = "Extrayendo ffmpeg…".into();
        i.progress = -1.0;
    });
    let res = (|| -> Result<(), String> {
        let mut z = zip::ZipArchive::new(File::open(&zip_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mut found = 0;
        for i in 0..z.len() {
            let mut e = z.by_index(i).map_err(|e| e.to_string())?;
            let lower = e.name().to_lowercase();
            let fname = if lower.ends_with("/bin/ffmpeg.exe") {
                "ffmpeg.exe"
            } else if lower.ends_with("/bin/ffprobe.exe") {
                "ffprobe.exe"
            } else if lower.ends_with("/bin/ffplay.exe") {
                "ffplay.exe"
            } else {
                continue;
            };
            let tmp = bin.join(format!("{fname}.new"));
            let mut out = File::create(&tmp).map_err(|e| e.to_string())?;
            std::io::copy(&mut e, &mut out).map_err(|e| e.to_string())?;
            drop(out);
            replace_file(&tmp, &bin.join(fname))?;
            found += 1;
        }
        if found < 2 { Err("el zip de ffmpeg no contiene ffmpeg.exe y ffprobe.exe".into()) } else { Ok(()) }
    })();
    let _ = std::fs::remove_file(&zip_path);
    res
}

fn marker_path() -> PathBuf {
    paths::data_dir().join("tools.json")
}

fn read_marker() -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(marker_path()).ok()?).ok()?;
    v["ffmpeg_release"].as_str().map(String::from)
}

fn write_marker(release: &str) {
    crate::config::write_json(&marker_path(), &serde_json::json!({ "ffmpeg_release": release }));
}

/// Comprueba (y opcionalmente actualiza) yt-dlp y ffmpeg. Bloqueante: llamar desde un hilo.
/// Instala siempre lo que falte; solo actualiza lo desactualizado si `update_outdated`.
pub fn check_and_update(info: &Mutex<ToolsInfo>, update_outdated: bool) {
    set(info, |i| {
        i.busy = true;
        i.progress = -1.0;
        i.status = "Comprobando herramientas…".into();
    });
    migrate_local_copies();

    // ---- yt-dlp ----
    let mut installed = ytdlp_version();
    let latest = latest_ytdlp();
    let offline = latest.is_err();
    set(info, |i| {
        i.ytdlp = installed.clone().unwrap_or_default();
        i.ytdlp_latest = latest.clone().unwrap_or_default();
    });
    let need = match (&installed, &latest) {
        (None, _) => true,
        (Some(cur), Ok(l)) => update_outdated && cur.as_str() < l.as_str(),
        _ => false,
    };
    if need {
        match install_ytdlp(info) {
            Ok(()) => {
                installed = ytdlp_version();
                log::info!("yt-dlp actualizado a {:?}", installed);
            }
            Err(e) => log::warn!("No se pudo instalar yt-dlp: {e}"),
        }
        set(info, |i| i.ytdlp = installed.clone().unwrap_or_default());
    }

    // ---- ffmpeg ----
    let mut ff = ffmpeg_version();
    let ff_latest = latest_ffmpeg();
    set(info, |i| {
        i.ffmpeg = ff.clone().unwrap_or_default();
        i.ffmpeg_latest = ff_latest.clone().unwrap_or_default();
    });
    let need_ff = match (&ff, &ff_latest) {
        (None, _) => true,
        (Some(_), Ok(_)) if !paths::bin_dir().join("ffplay.exe").is_file() => true,
        (Some(cur), Ok(l)) => update_outdated && cur != l && read_marker().as_deref() != Some(l.as_str()),
        _ => false,
    };
    if need_ff {
        match install_ffmpeg(info) {
            Ok(()) => {
                if let Ok(l) = &ff_latest {
                    write_marker(l);
                }
                ff = ffmpeg_version();
                log::info!("ffmpeg actualizado a {:?}", ff);
            }
            Err(e) => log::warn!("No se pudo instalar ffmpeg: {e}"),
        }
        set(info, |i| i.ffmpeg = ff.clone().unwrap_or_default());
    }

    let msg = if installed.is_none() || ff.is_none() {
        if offline {
            "Faltan herramientas y no hay conexión para descargarlas".to_string()
        } else {
            "No se pudieron instalar todas las herramientas (revisa el log)".to_string()
        }
    } else if offline {
        "Sin conexión: se usan las versiones locales".to_string()
    } else if need || need_ff {
        "Herramientas actualizadas".to_string()
    } else {
        "Herramientas al día".to_string()
    };
    log::info!("Herramientas: {msg}");
    set(info, |i| {
        i.busy = false;
        i.progress = 0.0;
        i.status = msg;
    });
}
