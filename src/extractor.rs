//! Extraccion de metadatos (sin descargar) con yt-dlp --dump-single-json.
use crate::args::network_args;
use crate::config::Settings;
use crate::model::{Extracted, Skipped, Video};
use crate::{tools, util};
use serde_json::Value;
use std::io::Read;
use std::process::Stdio;
use std::time::{Duration, Instant};

const PRIVATE_MARKERS: [&str; 9] = [
    "private video", "video unavailable", "this video is unavailable", "has been removed",
    "deleted video", "members-only", "login required", "blocked due to the claimed content",
    "copyright claim",
];

/// Ejecuta yt-dlp con tiempo limite; devuelve (stdout, stderr, codigo).
fn run_capture(args: &[String], timeout: Duration) -> Result<(Vec<u8>, String, i32), String> {
    let mut child = util::command(tools::ytdlp_path())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("No se pudo ejecutar yt-dlp: {e}"))?;
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let t_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out.read_to_end(&mut b);
        b
    });
    let t_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err.read_to_end(&mut b);
        b
    });
    let start = Instant::now();
    let code = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st.code().unwrap_or(-1),
            Ok(None) if start.elapsed() > timeout => {
                util::kill_tree(child.id());
                let _ = child.wait();
                return Err("Tiempo de espera agotado consultando el enlace".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let stdout = t_out.join().unwrap_or_default();
    let stderr = String::from_utf8_lossy(&t_err.join().unwrap_or_default()).into_owned();
    Ok((stdout, stderr, code))
}

fn last_error(stderr: &str) -> String {
    stderr
        .lines()
        .rev()
        .find(|l| l.contains("ERROR"))
        .or_else(|| stderr.lines().rev().find(|l| !l.trim().is_empty()))
        .map(|l| l.trim().trim_start_matches("ERROR:").trim().to_string())
        .unwrap_or_else(|| "yt-dlp falló al extraer información".into())
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(String::from)
}

fn unavailable(e: &Value) -> bool {
    let title = str_of(e, "title").unwrap_or_default().to_lowercase();
    if title.is_empty() && e.get("duration").map_or(true, Value::is_null) {
        return true;
    }
    e.get("availability").and_then(Value::as_str) == Some("private")
        || PRIVATE_MARKERS.iter().any(|m| title.contains(m))
        || title == "[private video]"
}

fn thumbnail_for(e: &Value, id: &str, url: &str, extractor: &str) -> Option<String> {
    let is_yt = extractor.to_lowercase().contains("youtube") || url.contains("youtube.com") || url.contains("youtu.be");
    if is_yt && !id.is_empty() {
        return Some(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"));
    }
    str_of(e, "thumbnail").or_else(|| {
        e.get("thumbnails")?
            .as_array()?
            .iter()
            .rev()
            .find_map(|t| str_of(t, "url"))
    })
}

/// Si las cookies del navegador fallan (base de datos bloqueada), reintenta sin cookies.
pub fn extract(url: &str, s: &Settings, single: bool) -> Result<Extracted, String> {
    match extract_inner(url, s, single) {
        Err(m) if s.cookies_mode == "browser" && m.to_lowercase().contains("cookie") => {
            log::warn!("cookies del navegador fallaron ({m}); se reintenta sin cookies");
            let mut s2 = s.clone();
            s2.cookies_mode = "off".into();
            extract_inner(url, &s2, single)
        }
        r => r,
    }
}

fn extract_inner(url: &str, s: &Settings, single: bool) -> Result<Extracted, String> {
    let mut args: Vec<String> = ["--dump-single-json", "--no-warnings", "--flat-playlist", "--encoding", "utf-8"]
        .iter()
        .map(|x| x.to_string())
        .collect();
    if single {
        args.push("--no-playlist".into());
    } else if !url.starts_with("ytsearch") {
        args.extend(["--playlist-end".into(), s.playlist_scan_limit.max(1).to_string()]);
    }
    args.extend(network_args(s));
    args.push(url.into());

    let (stdout, stderr, code) = run_capture(&args, Duration::from_secs(150))?;
    if code != 0 {
        let msg = last_error(&stderr);
        log::warn!("extraccion fallida para {url}: {msg}");
        return Err(msg);
    }
    let info: Value = serde_json::from_slice(&stdout).map_err(|e| format!("Respuesta de yt-dlp inválida: {e}"))?;
    let is_playlist = info.get("_type").and_then(Value::as_str) == Some("playlist");
    let entries: Vec<Value> = if is_playlist {
        info.get("entries").and_then(Value::as_array).cloned().unwrap_or_default()
    } else {
        vec![info.clone()]
    };

    let mut out = Extracted { playlist_title: is_playlist.then(|| str_of(&info, "title")).flatten(), ..Default::default() };
    for e in &entries {
        if e.is_null() {
            out.skipped.push(Skipped { title: "Elemento no disponible".into(), reason: "No se pudo acceder al video".into() });
            continue;
        }
        if unavailable(e) {
            out.skipped.push(Skipped {
                title: str_of(e, "title").unwrap_or_else(|| "Video privado o no disponible".into()),
                reason: "Video privado, eliminado o no disponible".into(),
            });
            continue;
        }
        let id = str_of(e, "id").unwrap_or_default();
        let vurl = if is_playlist {
            str_of(e, "url").or_else(|| str_of(e, "webpage_url"))
        } else {
            str_of(e, "webpage_url").or_else(|| str_of(e, "original_url"))
        }
        .unwrap_or_else(|| url.to_string());
        let extractor = str_of(e, "ie_key").or_else(|| str_of(e, "extractor_key")).or_else(|| str_of(e, "extractor")).unwrap_or_default();
        out.items.push(Video {
            thumbnail: thumbnail_for(e, &id, &vurl, &extractor),
            id,
            title: str_of(e, "title").unwrap_or_else(|| "Sin título".into()),
            url: vurl,
            duration: e.get("duration").and_then(Value::as_f64),
            extractor: extractor.to_lowercase(),
            channel: str_of(e, "channel").or_else(|| str_of(e, "uploader")).unwrap_or_default(),
            playlist: String::new(),
        });
    }
    Ok(out)
}
