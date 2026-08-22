"""Rutas base, settings.json y profiles.json (lectura/escritura y valores por defecto)."""
import os
import sys
import glob
import json

APP_NAME = "ClipVault"


def _is_frozen():
    return getattr(sys, "frozen", False)


# --------------------------------------------------------------------------
# Carpeta de DATOS DE USUARIO (settings.json, profiles.json, history.json,
# logs). SIEMPRE en %LOCALAPPDATA%\ClipVault, que es escribible sin permisos
# de administrador, sin importar donde se instalo el programa (Program Files
# requiere admin para escribir; AppData\Local nunca).
# --------------------------------------------------------------------------
USER_DATA_DIR = os.path.join(
    os.environ.get("LOCALAPPDATA") or os.path.expanduser("~"), APP_NAME
)
os.makedirs(USER_DATA_DIR, exist_ok=True)

SETTINGS_PATH = os.path.join(USER_DATA_DIR, "settings.json")
PROFILES_PATH = os.path.join(USER_DATA_DIR, "profiles.json")
HISTORY_PATH = os.path.join(USER_DATA_DIR, "history.json")

LOG_DIR = os.path.join(USER_DATA_DIR, "logs")
os.makedirs(LOG_DIR, exist_ok=True)

# --------------------------------------------------------------------------
# Carpeta de INSTALACION (donde vive el .exe y los binarios empaquetados,
# como ffmpeg.exe / ffprobe.exe / yt-dlp.exe). Solo se usa para LOCALIZAR
# recursos, nunca para guardar nada -- puede ser de solo lectura.
# --------------------------------------------------------------------------
if _is_frozen():
    APP_DIR = os.path.dirname(sys.executable)
else:
    APP_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Alias retrocompatible: codigo viejo (p. ej. core/logger.py) puede seguir
# importando BASE_DIR. Apunta a la carpeta de instalacion, igual que antes.
BASE_DIR = APP_DIR

# Carpeta de descargas por defecto: la Descargas real del usuario actual,
# nunca una ruta fija de otra persona ni una carpeta dentro del programa.
DEFAULT_DOWNLOAD_DIR = os.path.join(os.path.expanduser("~"), "Downloads")
os.makedirs(DEFAULT_DOWNLOAD_DIR, exist_ok=True)

MIN_FREE_BYTES = 200 * 1024 * 1024


def bundled_ffmpeg_dir():
    """Busca ffmpeg.exe junto al .exe principal (asi se distribuye ahora)."""
    try:
        candidate = os.path.join(APP_DIR, "ffmpeg.exe")
        if os.path.isfile(candidate):
            return APP_DIR
    except Exception:
        pass
    return ""


def autodetect_winget_ffmpeg():
    """Fallback solo para entorno de desarrollo (ffmpeg instalado via winget)."""
    try:
        local_appdata = os.environ.get("LOCALAPPDATA", "")
        if not local_appdata:
            return ""
        pattern = os.path.join(
            local_appdata, "Microsoft", "WinGet", "Packages",
            "Gyan.FFmpeg*", "ffmpeg-*-full_build", "bin"
        )
        matches = glob.glob(pattern)
        return matches[0] if matches else ""
    except Exception:
        return ""


def detect_ffmpeg_location():
    """
    Orden de prioridad:
    1. Variable de entorno FFMPEG_LOCATION (si el usuario la definio a mano).
    2. Junto al .exe principal (caso normal para cualquier usuario final).
    3. WinGet local (solo utile en la maquina del desarrollador).
    """
    env_path = os.environ.get("FFMPEG_LOCATION", "")
    if env_path and os.path.isdir(env_path):
        return env_path
    bundled = bundled_ffmpeg_dir()
    if bundled:
        return bundled
    return autodetect_winget_ffmpeg()


# Alias retrocompatibles con guion bajo, por si algun modulo viejo los usa asi.
_bundled_ffmpeg_dir = bundled_ffmpeg_dir
_autodetect_winget_ffmpeg = autodetect_winget_ffmpeg
_detect_ffmpeg_location = detect_ffmpeg_location


def default_download_dir():
    return DEFAULT_DOWNLOAD_DIR


_default_download_dir = default_download_dir


DEFAULT_SETTINGS = {
    "theme": "dark",
    "download_dir": DEFAULT_DOWNLOAD_DIR,
    "ffmpeg_location": detect_ffmpeg_location(),
    "cookies_mode": "off", "cookies_file": "", "cookies_browser": "chrome",
    "proxy_url": "", "playlist_scan_limit": 300,
    "cautious_mode": "balanced",
    "active_profile_id": "audio_mp3",
    "rate_limit_kbps": 0,
    "concurrency": 2,
    "notifications_enabled": True,
    "normalize_volume": False,
    "sponsorblock_remove": False,
    "embed_thumbnail_metadata": False,
    "download_subtitles": False,
    "subtitle_langs": "es,en",
}


def load_json(path, default):
    if os.path.exists(path):
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
            return {**default, **data} if isinstance(default, dict) else data
        except Exception:
            pass
    return default.copy() if isinstance(default, dict) else default


def save_json(path, data):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)


def get_settings():
    s = load_json(SETTINGS_PATH, DEFAULT_SETTINGS)
    # Si la ruta de ffmpeg guardada ya no existe (por ejemplo, se copio de
    # otra maquina), se vuelve a detectar en caliente en vez de fallar.
    loc = s.get("ffmpeg_location", "")
    if loc and not os.path.isdir(loc):
        s["ffmpeg_location"] = detect_ffmpeg_location()
    return s


def save_settings(s):
    save_json(SETTINGS_PATH, s)


DEFAULT_PROFILES = [
    {"id": "best", "name": "Best available", "builtin": True, "kind": "video", "height": None, "container": "native", "desc": "Best available - Native formats - Native audio"},
    {"id": "2160p", "name": "4K UHD 2160p", "builtin": True, "kind": "video", "height": 2160, "container": "native", "desc": "Up to 2160p - Native formats - Native audio"},
    {"id": "1440p", "name": "QHD 1440p", "builtin": True, "kind": "video", "height": 1440, "container": "native", "desc": "Up to 1440p - Native formats - Native audio"},
    {"id": "1080p", "name": "Full HD 1080p", "builtin": True, "kind": "video", "height": 1080, "container": "native", "desc": "Up to 1080p - Native formats - Native audio"},
    {"id": "720p", "name": "Balanced 720p", "builtin": True, "kind": "video", "height": 720, "container": "native", "desc": "Up to 720p - Native formats - Native audio"},
    {"id": "480p", "name": "Small file 480p", "builtin": True, "kind": "video", "height": 480, "container": "native", "desc": "Up to 480p - Native formats - Native audio"},
    {"id": "tv1080", "name": "Smart TV MP4 Full HD 1080p", "builtin": True, "kind": "video", "height": 1080, "container": "mp4", "desc": "Up to 1080p - MP4/H.264 - AAC audio"},
    {"id": "tv720", "name": "Smart TV MP4 HD 720p", "builtin": True, "kind": "video", "height": 720, "container": "mp4", "desc": "Up to 720p - MP4/H.264 - AAC audio"},
    {"id": "tv480", "name": "Smart TV MP4 SD 480p", "builtin": True, "kind": "video", "height": 480, "container": "mp4", "desc": "Up to 480p - MP4/H.264 - AAC audio"},
    {"id": "audio_mp3", "name": "Audio MP3", "builtin": True, "kind": "audio", "audio_codec": "mp3", "desc": "Audio only - MP3 192kbps - Maxima compatibilidad"},
    {"id": "audio_m4a", "name": "Audio M4A / AAC", "builtin": True, "kind": "audio", "audio_codec": "m4a", "desc": "Audio only - M4A/AAC 192kbps - Ideal iPhone/iTunes"},
    {"id": "audio_wav", "name": "Audio WAV", "builtin": True, "kind": "audio", "audio_codec": "wav", "desc": "Audio only - WAV sin perdida - Archivo pesado"},
    {"id": "audio_flac", "name": "Audio FLAC", "builtin": True, "kind": "audio", "audio_codec": "flac", "desc": "Audio only - FLAC sin perdida - Comprimido"},
    {"id": "audio_opus", "name": "Audio Opus (original)", "builtin": True, "kind": "audio", "audio_codec": "opus", "desc": "Audio only - Opus nativo de YouTube - Sin recodificar"},
]


def get_profiles():
    return load_json(PROFILES_PATH, {"profiles": DEFAULT_PROFILES})["profiles"] \
        if os.path.exists(PROFILES_PATH) else DEFAULT_PROFILES


def save_profiles(profiles):
    save_json(PROFILES_PATH, {"profiles": profiles})
