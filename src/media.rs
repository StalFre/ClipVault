//! Vista previa (ffplay) y edicion de etiquetas (ffmpeg/ffprobe).
use crate::config::Settings;
use crate::model::Video;
use crate::{args, paths, util};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;

static PREVIEW_PID: Mutex<Option<u32>> = Mutex::new(None);

pub fn ffplay_path() -> Option<PathBuf> {
    let p = paths::bin_dir().join("ffplay.exe");
    p.is_file().then_some(p)
}

pub fn stop_preview() {
    if let Some(pid) = PREVIEW_PID.lock().ok().and_then(|mut g| g.take()) {
        util::kill_tree(pid);
    }
}

/// Reproduce ~10 s del video (o solo audio). Bloqueante: llamar desde un hilo.
/// Devuelve un mensaje de error si algo falla.
static STREAM_CACHE: Mutex<Option<(String, bool, String)>> = Mutex::new(None);

/// URL directa del flujo (con reintentos sin cookies). Se cachea el ultimo para no repetir la consulta.
pub fn stream_url(v: &Video, s: &Settings, with_video: bool) -> Result<String, String> {
    if let Some((u, w, st)) = STREAM_CACHE.lock().ok().and_then(|g| g.clone()) {
        if u == v.url && w == with_video {
            return Ok(st);
        }
    }
    let fmt = if with_video { "b[height<=480]/b" } else { "ba/b" };
    // Con cookies del navegador puede fallar (base de datos bloqueada): se reintenta sin ellas y con otros formatos.
    let mut attempts: Vec<(Settings, &str)> = vec![(s.clone(), fmt)];
    if s.cookies_mode != "off" {
        let mut s2 = s.clone();
        s2.cookies_mode = "off".into();
        attempts.push((s2, fmt));
    }
    let fallback = if with_video { "best" } else { "bestaudio/best" };
    let mut s3 = s.clone();
    s3.cookies_mode = "off".into();
    attempts.push((s3, fallback));
    let mut stream = String::new();
    let mut last_err = String::new();
    for (st, f) in &attempts {
        let mut a: Vec<String> = vec!["-g".into(), "-f".into(), (*f).into(), "--no-playlist".into(), "--no-warnings".into()];
        a.extend(args::network_args(st));
        a.push(v.url.clone());
        let out = util::command(crate::tools::ytdlp_path())
            .args(&a)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("No se pudo ejecutar yt-dlp: {e}"))?;
        stream = String::from_utf8_lossy(&out.stdout).lines().find(|l| l.starts_with("http")).unwrap_or("").trim().to_string();
        if !stream.is_empty() {
            break;
        }
        last_err = String::from_utf8_lossy(&out.stderr).lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    }
    if stream.is_empty() {
        log::warn!("vista previa sin flujo para {}: {last_err}", v.url);
        return Err(if last_err.is_empty() { "No se pudo obtener el flujo para la vista previa".into() } else { last_err });
    }
    if let Ok(mut g) = STREAM_CACHE.lock() {
        *g = Some((v.url.clone(), with_video, stream.clone()));
    }
    Ok(stream)
}

/// Un fotograma del video en el segundo indicado, como RGBA.
pub fn frame_at(v: &Video, s: &Settings, secs: u32) -> Result<(Vec<u8>, u32, u32), String> {
    let stream = stream_url(v, s, true)?;
    let out = util::command(paths::bin_dir().join("ffmpeg.exe"))
        .args(["-v", "error", "-ss", &secs.to_string(), "-i", &stream, "-frames:v", "1", "-vf", "scale=640:-2", "-f", "image2pipe", "-vcodec", "png", "-"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if out.stdout.is_empty() {
        return Err("No se pudo obtener el fotograma".into());
    }
    let img = image::load_from_memory(&out.stdout).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = img.dimensions();
    Ok((img.into_raw(), w, h))
}

pub fn preview(v: &Video, s: &Settings, with_video: bool, start_at: Option<u32>) -> Result<(), String> {
    stop_preview();
    let ffplay = ffplay_path().ok_or("Falta ffplay: pulsa «Buscar actualizaciones» en Ajustes")?;
    let stream = stream_url(v, s, with_video)?;
    let start = start_at.unwrap_or_else(|| v.duration.filter(|d| *d > 40.0).map(|d| (d * 0.25) as u32).unwrap_or(0));
    let mut f: Vec<String> = ["-autoexit", "-loglevel", "quiet", "-t", "10"].iter().map(|s| s.to_string()).collect();
    if start > 0 {
        f.extend(["-ss".into(), start.to_string()]);
    }
    if with_video {
        f.extend(["-x".into(), "640".into(), "-window_title".into(), format!("Vista previa · {}", v.title)]);
    } else {
        f.push("-nodisp".into());
    }
    f.push(stream);
    let mut child = util::command(ffplay).args(&f).stdin(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    if let Ok(mut g) = PREVIEW_PID.lock() {
        *g = Some(child.id());
    }
    let _ = child.wait();
    if let Ok(mut g) = PREVIEW_PID.lock() {
        if *g == Some(child.id()) {
            *g = None;
        }
    }
    Ok(())
}

#[derive(Default, Clone, Debug)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album: String,
}

pub fn read_tags(file: &Path) -> Tags {
    let ffprobe = paths::bin_dir().join("ffprobe.exe");
    let out = util::command(ffprobe)
        .args(["-v", "quiet", "-print_format", "json", "-show_format"])
        .arg(file)
        .output()
        .ok();
    let v: serde_json::Value = out.and_then(|o| serde_json::from_slice(&o.stdout).ok()).unwrap_or_default();
    let tags = &v["format"]["tags"];
    let get = |k: &str| {
        tags.as_object()
            .and_then(|m| m.iter().find(|(key, _)| key.eq_ignore_ascii_case(k)))
            .and_then(|(_, val)| val.as_str())
            .unwrap_or("")
            .to_string()
    };
    Tags { title: get("title"), artist: get("artist"), album: get("album") }
}

/// Escribe etiquetas (y portada opcional) sin recodificar el audio.
pub fn write_tags(file: &Path, t: &Tags, cover: Option<&Path>) -> Result<(), String> {
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("mp3").to_lowercase();
    let ffmpeg = paths::bin_dir().join("ffmpeg.exe");
    let tmp = file.with_extension(format!("cvtmp.{ext}"));
    let use_cover = cover.is_some() && matches!(ext.as_str(), "mp3" | "m4a" | "flac" | "mp4");
    let mut c = util::command(ffmpeg);
    c.args(["-y", "-v", "error", "-i"]).arg(file);
    if use_cover {
        c.arg("-i").arg(cover.unwrap());
        c.args(["-map", "0:a", "-map", "1:v"]);
    } else {
        c.args(["-map", "0"]);
    }
    c.args(["-c", "copy"]);
    if ext == "mp3" {
        c.args(["-id3v2_version", "3"]);
    }
    if use_cover {
        c.args(["-disposition:v:0", "attached_pic"]);
    }
    c.args(["-metadata", &format!("title={}", t.title), "-metadata", &format!("artist={}", t.artist), "-metadata", &format!("album={}", t.album)]);
    c.arg(&tmp);
    let out = c.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("ffmpeg falló").to_string());
    }
    std::fs::remove_file(file).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file).map_err(|e| e.to_string())
}

/// "Artista - Titulo" -> (artista, titulo)
pub fn guess_artist_title(name: &str) -> (String, String) {
    match name.split_once(" - ") {
        Some((a, t)) => (a.trim().to_string(), t.trim().to_string()),
        None => (String::new(), name.trim().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn live_tags_roundtrip() {
        let f = std::env::temp_dir().join("cv_test_out").join("Me at the zoo.mp3");
        let t = Tags { title: "Zoo".into(), artist: "Jawed".into(), album: "Alb".into() };
        write_tags(&f, &t, None).unwrap();
        let r = read_tags(&f);
        assert_eq!((r.title.as_str(), r.artist.as_str(), r.album.as_str()), ("Zoo", "Jawed", "Alb"));
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    #[test]
    #[ignore]
    fn live_preview_audio() {
        let v = Video { url: "https://www.youtube.com/watch?v=jNQXAC9IVRw".into(), title: "zoo".into(), duration: Some(19.0), ..Default::default() };
        let s = crate::config::load_settings();
        println!("cookies_mode={}", s.cookies_mode);
        preview(&v, &s, false, Some(0)).unwrap();
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    #[test]
    #[ignore]
    fn live_frame() {
        let v = Video { url: "https://www.youtube.com/watch?v=jNQXAC9IVRw".into(), ..Default::default() };
        let (px, w, h) = frame_at(&v, &crate::config::load_settings(), 5).unwrap();
        assert_eq!(px.len(), (w * h * 4) as usize);
        println!("{w}x{h}");
    }
}
