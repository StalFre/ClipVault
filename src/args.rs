//! Construccion de argumentos de yt-dlp (descarga y extraccion).
use crate::config::Settings;
use crate::model::Profile;

const SPONSOR_CATS: &str = "sponsor,selfpromo,interaction,intro,outro,preview,music_offtopic";

pub fn network_args(s: &Settings) -> Vec<String> {
    let mut a = Vec::new();
    if !s.proxy_url.trim().is_empty() {
        a.push("--proxy".into());
        a.push(s.proxy_url.trim().into());
    }
    match s.cookies_mode.as_str() {
        "file" if !s.cookies_file.trim().is_empty() => {
            a.push("--cookies".into());
            a.push(s.cookies_file.trim().into());
        }
        "browser" if !s.cookies_browser.is_empty() => {
            a.push("--cookies-from-browser".into());
            a.push(s.cookies_browser.clone());
        }
        _ => {}
    }
    a
}

fn format_for(p: &Profile) -> String {
    let hf = p.height.map(|h| format!("[height<={h}]")).unwrap_or_default();
    if p.container == "mp4" {
        format!(
            "bestvideo[vcodec^=avc1]{hf}+bestaudio[ext=m4a]/bestvideo[ext=mp4]{hf}+bestaudio[ext=m4a]/best[ext=mp4]{hf}/bestvideo{hf}+bestaudio/best{hf}"
        )
    } else {
        format!("bestvideo{hf}+bestaudio/best{hf}")
    }
}

/// `out_template` ya debe tener los `%` del titulo escapados.
pub fn download_args(
    p: &Profile,
    s: &Settings,
    out_template: &str,
    url: &str,
    ffmpeg_dir: Option<&str>,
    section: Option<&str>,
) -> Vec<String> {
    let mut a: Vec<String> = [
        "--no-playlist", "--newline", "--no-colors", "--windows-filenames", "--encoding", "utf-8",
        "--retries", "3", "--fragment-retries", "5", "--continue",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    a.push("-o".into());
    a.push(out_template.into());
    if let Some(dir) = ffmpeg_dir {
        a.push("--ffmpeg-location".into());
        a.push(dir.into());
    }
    if s.rate_limit_kbps > 0 {
        a.push("--limit-rate".into());
        a.push(format!("{}K", s.rate_limit_kbps));
    }
    a.extend(network_args(s));
    match s.cautious_mode.as_str() {
        "balanced" => a.extend(["--sleep-interval", "1", "--max-sleep-interval", "3"].map(String::from)),
        "careful" => a.extend(["--sleep-interval", "3", "--max-sleep-interval", "8"].map(String::from)),
        _ => {}
    }

    if p.is_audio() {
        let codec = if p.audio_codec.is_empty() { "mp3" } else { p.audio_codec.as_str() };
        a.extend(["-f".into(), "bestaudio/best".into(), "-x".into(), "--audio-format".into(), codec.into()]);
        if matches!(codec, "mp3" | "m4a") {
            a.extend(["--audio-quality".into(), "192K".into()]);
        }
    } else {
        a.extend(["-f".into(), format_for(p), "--merge-output-format".into(), "mp4".into()]);
    }

    if s.sponsorblock_remove {
        a.extend(["--sponsorblock-remove".into(), SPONSOR_CATS.into()]);
    }
    if s.normalize_volume {
        a.extend(["--postprocessor-args".into(), "ffmpeg:-af loudnorm=I=-16:TP=-1.5:LRA=11".into()]);
    }
    if s.embed_thumbnail_metadata {
        a.push("--embed-metadata".into());
        // WAV no admite imagen incrustada: yt-dlp fallaria.
        if !(p.is_audio() && p.audio_codec == "wav") {
            a.extend(["--embed-thumbnail".into(), "--convert-thumbnails".into(), "jpg".into()]);
        }
    }
    if s.download_subtitles && !p.is_audio() {
        let langs = if s.subtitle_langs.trim().is_empty() { "en" } else { s.subtitle_langs.trim() };
        a.extend([
            "--write-subs".into(), "--write-auto-subs".into(), "--sub-langs".into(), langs.into(),
            "--convert-subs".into(), "srt".into(),
        ]);
    }
    if let Some(sec) = section {
        a.extend(["--download-sections".into(), sec.into(), "--force-keyframes-at-cuts".into()]);
    }
    a.push(url.into());
    a
}

/// Extension final esperada para perfiles de audio (para verificar que se convirtio).
pub fn expected_audio_ext(p: &Profile) -> Option<&str> {
    p.is_audio().then(|| if p.audio_codec.is_empty() { "mp3" } else { p.audio_codec.as_str() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::builtin_profiles;

    #[test]
    fn mp3_profile_extracts_mp3() {
        let p = builtin_profiles().into_iter().find(|p| p.id == "audio_mp3").unwrap();
        let a = download_args(&p, &Settings::default(), "x.%(ext)s", "u", None, None);
        let i = a.iter().position(|s| s == "--audio-format").unwrap();
        assert_eq!(a[i + 1], "mp3");
        assert!(a.contains(&"-x".to_string()));
    }
}
