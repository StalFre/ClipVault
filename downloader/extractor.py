"""Extraccion de metadata (sin descargar) usando el binario yt-dlp.exe
via subprocess, en vez de la libreria yt_dlp. Mantiene exactamente el
mismo contrato de datos que antes: extract_info(url, settings, single)
devuelve {"playlist_title", "items", "skipped", "total_count"}.
"""
from downloader.ytdlp_bin import extract_info as ytdlp_extract_info

YOUTUBE_MARKERS = ["youtube", "youtu.be"]
PRIVATE_MARKERS = [
    "private video", "private", "video unavailable", "this video is unavailable",
    "has been removed", "account associated with this video has been terminated",
    "members-only", "this live stream recording is not available",
    "login required", "sign in to confirm", "blocked due to the claimed content",
    "claimed content by", "copyright claim",
]


def _fallback_thumbnail(video_id, url, extractor):
    is_youtube = (
        any(m in (extractor or "").lower() for m in YOUTUBE_MARKERS)
        or any(m in (url or "").lower() for m in YOUTUBE_MARKERS)
    )
    if is_youtube and video_id:
        return f"https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"
    return None


def _is_entry_unavailable(entry: dict) -> bool:
    if not entry:
        return True
    title = (entry.get("title") or "").lower()
    if title in ("[private video]", "private video", "[deleted video]", "deleted video"):
        return True
    for marker in PRIVATE_MARKERS:
        if marker in title:
            return True
    if entry.get("availability") == "private":
        return True
    if not entry.get("title") and not entry.get("duration"):
        return True
    return False


def extract_info(url: str, settings: dict, single: bool):
    extra_args = ["--flat-playlist"]
    if single:
        extra_args.append("--no-playlist")
    else:
        limit = settings.get("playlist_scan_limit", 300)
        extra_args += ["--playlist-end", str(limit if not single else 1)]

    if settings.get("proxy_url"):
        extra_args += ["--proxy", settings["proxy_url"]]
    mode = settings.get("cookies_mode", "off")
    if mode == "file" and settings.get("cookies_file"):
        extra_args += ["--cookies", settings["cookies_file"]]
    elif mode == "browser" and settings.get("cookies_browser"):
        extra_args += ["--cookies-from-browser", settings["cookies_browser"]]

    info = ytdlp_extract_info(url, extra_args=extra_args)

    entries = info.get("entries") if info.get("_type") == "playlist" else [info]
    items, skipped = [], []
    total_count = 0

    for e in entries:
        if not e:
            total_count += 1
            skipped.append({"title": "Elemento no disponible", "reason": "No se pudo acceder al video", "url": url})
            continue
        total_count += 1
        if _is_entry_unavailable(e):
            title = e.get("title") or "Video privado o no disponible"
            vid_url = e.get("url") or e.get("webpage_url") or url
            skipped.append({"title": title, "reason": "Video privado, eliminado o no disponible", "url": vid_url})
            continue

        vid_id = e.get("id")
        vid_url = e.get("url") or e.get("webpage_url") or url
        extractor = e.get("ie_key") or e.get("extractor") or ""
        thumb = e.get("thumbnail")
        if not thumb:
            thumbs_list = e.get("thumbnails") or []
            if thumbs_list:
                thumb = thumbs_list[-1].get("url")
        if not thumb:
            thumb = _fallback_thumbnail(vid_id, vid_url, extractor)

        items.append({
            "id": vid_id,
            "title": e.get("title") or "Sin titulo",
            "url": vid_url,
            "thumbnail": thumb,
            "duration": e.get("duration"),
            "extractor": extractor.lower(),
        })

    return {
        "playlist_title": info.get("title") if info.get("_type") == "playlist" else None,
        "items": items,
        "skipped": skipped,
        "total_count": total_count,
    }
