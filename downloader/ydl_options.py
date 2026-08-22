"""Construccion de las opciones que se le pasan a yt-dlp: perfil de calidad,
opciones comunes (proxy, cookies, ffmpeg, velocidad) y opciones avanzadas
(SponsorBlock, metadata, subtitulos).
"""

AUDIO_CODEC_QUALITY = {"mp3": "192", "m4a": "192", "wav": None, "flac": None, "opus": None, "vorbis": "192"}
SPONSORBLOCK_CATEGORIES = ["sponsor", "selfpromo", "interaction", "intro", "outro", "preview", "music_offtopic"]


def profile_to_ytdlp_opts(profile: dict):
    if profile["kind"] == "audio":
        codec = profile.get("audio_codec") or "mp3"
        pp = {"key": "FFmpegExtractAudio", "preferredcodec": codec}
        quality = AUDIO_CODEC_QUALITY.get(codec)
        if quality:
            pp["preferredquality"] = quality
        return {"format": "bestaudio/best", "postprocessors": [pp]}
    height = profile.get("height")
    hf = f"[height<={height}]" if height else ""
    if profile.get("container") == "mp4":
        fmt = f"bestvideo[ext=mp4]{hf}+bestaudio[ext=m4a]/best[ext=mp4]{hf}/bestvideo{hf}+bestaudio/best{hf}"
        return {"format": fmt, "merge_output_format": "mp4"}
    fmt = f"bestvideo{hf}+bestaudio/best{hf}" if height else "bestvideo+bestaudio/best"
    return {"format": fmt, "merge_output_format": "mp4"}


from core.config import bundled_ffmpeg_dir

def common_ydl_opts(settings: dict):
    opts = {"windowsfilenames": True}
    ffmpeg_loc = settings.get("ffmpeg_location") or bundled_ffmpeg_dir()
    if ffmpeg_loc:
        opts["ffmpeg_location"] = ffmpeg_loc
    rate_kbps = settings.get("rate_limit_kbps", 0)
    if rate_kbps and rate_kbps > 0:
        opts["ratelimit"] = int(rate_kbps) * 1024
    if settings.get("proxy_url"):
        opts["proxy"] = settings["proxy_url"]
    mode = settings.get("cookies_mode", "off")
    if mode == "file" and settings.get("cookies_file"):
        opts["cookiefile"] = settings["cookies_file"]
    elif mode == "browser" and settings.get("cookies_browser"):
        opts["cookiesfrombrowser"] = (settings["cookies_browser"],)
    return opts


def advanced_ydl_opts(settings: dict):
    opts = {"postprocessors": []}
    if settings.get("sponsorblock_remove"):
        opts["postprocessors"].append({"key": "SponsorBlock", "categories": SPONSORBLOCK_CATEGORIES})
        opts["postprocessors"].append({"key": "ModifyChapters", "remove_sponsor_segments": SPONSORBLOCK_CATEGORIES})
    if settings.get("normalize_volume"):
        opts["postprocessor_args"] = {"ffmpeg": ["-af", "loudnorm=I=-16:TP=-1.5:LRA=11"]}
    if settings.get("embed_thumbnail_metadata"):
        opts["writethumbnail"] = True
        opts["postprocessors"].append({"key": "FFmpegMetadata", "add_metadata": True})
        opts["postprocessors"].append({"key": "EmbedThumbnail"})
    if settings.get("download_subtitles"):
        langs = [s.strip() for s in (settings.get("subtitle_langs") or "es,en").split(",") if s.strip()]
        opts["writesubtitles"] = True
        opts["writeautomaticsub"] = True
        opts["subtitleslangs"] = langs or ["en"]
    
    return opts


def merge_postprocessors(base_opts, extra_opts):
    merged = dict(base_opts)
    extra_pp = extra_opts.pop("postprocessors", [])
    merged.update(extra_opts)
    merged["postprocessors"] = merged.get("postprocessors", []) + extra_pp
    return merged
