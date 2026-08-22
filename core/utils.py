"Utilidades generales: sanitizacion de nombres, deteccion de errores permanentes, notificaciones de escritorio y la excepcion interna de cancelacion."
import os
import re

try:
    from plyer import notification as desktop_notification
    HAS_PLYER = True
except Exception:
    HAS_PLYER = False

from core.config import get_settings

# Caracteres invalidos en sistemas de archivos Windows
_INVALID_WIN_CHARS = re.compile(r'[<>:"/\\|?*\x00-\x1f]')
_INVALID_UNIX_CHARS = re.compile(r'/')


def sanitize_filename(name: str, max_len: int = 180) -> str:
    "Sanitizacion explicita de nombre de archivo para evitar fallas silenciosas."
    if not name:
        return "video"
    name = _INVALID_WIN_CHARS.sub("_", name)
    name = _INVALID_UNIX_CHARS.sub("_", name)
    name = re.sub(r"\s+", " ", name).strip()
    name = name.strip(". ")
    if len(name) > max_len:
        name = name[:max_len].rstrip()
    return name or "video"


def build_job_download_dir(base_dir, profile_name, playlist_title):
    folder = os.path.join(base_dir, sanitize_filename(profile_name))
    if playlist_title:
        folder = os.path.join(folder, sanitize_filename(playlist_title))
    return folder


PERMANENT_ERROR_MARKERS = [
    "Private video", "Video unavailable", "This video is unavailable",
    "has been removed", "account associated with this video has been terminated",
    "copyright", "not available in your country", "members-only content",
    "This live stream recording is not available",
]


def is_permanent_error(err: str) -> bool:
    return any(m.lower() in err.lower() for m in PERMANENT_ERROR_MARKERS)


class CancelledError(Exception):
    pass


def notify_desktop(title, message):
    settings = get_settings()
    if not settings.get("notifications_enabled", True) or not HAS_PLYER:
        return
    try:
        desktop_notification.notify(title=title, message=message, timeout=6)
    except Exception:
        pass
