"""/api/ytdlp-version, /api/ytdlp-update, /api/install-ffmpeg, /api/check-ffmpeg."""
import os
import shutil
import threading
import subprocess

from flask import Blueprint, jsonify

from core.config import get_settings, save_settings, _autodetect_winget_ffmpeg
from core.logger import logger
from core.extensions import socketio
from downloader.ytdlp_bin import get_version, update_async

system_bp = Blueprint("system", __name__)


@system_bp.route("/api/check-ffmpeg")
def check_ffmpeg():
    settings = get_settings()
    loc = settings.get("ffmpeg_location") or ""
    on_path = shutil.which("ffmpeg")
    ok = bool(on_path) or bool(loc and os.path.isdir(loc))
    return jsonify({"ffmpeg_location": loc, "ffmpeg_on_path": on_path, "likely_ok": ok})


@system_bp.route("/api/install-ffmpeg", methods=["POST"])
def api_install_ffmpeg():
    def run_install():
        socketio.emit("ffmpeg_install_log", {"line": "Instalando ffmpeg con winget..."})
        try:
            proc = subprocess.Popen(
                ["winget", "install", "--id", "Gyan.FFmpeg", "-e",
                 "--accept-package-agreements", "--accept-source-agreements"],
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, creationflags=subprocess.CREATE_NO_WINDOW
            )
            for line in proc.stdout:
                socketio.emit("ffmpeg_install_log", {"line": line.rstrip()})
            proc.wait()
            ok = proc.returncode == 0
            found_path = _autodetect_winget_ffmpeg()
            if found_path:
                settings = get_settings()
                settings["ffmpeg_location"] = found_path
                save_settings(settings)
            if ok and found_path:
                message = "ffmpeg instalado y configurado correctamente."
            elif ok:
                message = "Instalacion terminada pero no se encontro la ruta automaticamente. Reinicia la app."
            else:
                message = f"winget termino con codigo {proc.returncode}"
            socketio.emit("ffmpeg_install_done", {"ok": ok, "path": found_path, "message": message})
        except FileNotFoundError:
            socketio.emit("ffmpeg_install_done", {"ok": False, "message": "winget no esta disponible en este sistema."})
        except Exception as exc:
            socketio.emit("ffmpeg_install_done", {"ok": False, "message": str(exc)})

    threading.Thread(target=run_install, daemon=True).start()
    return jsonify({"started": True})


@system_bp.route("/api/ytdlp-version")
def api_ytdlp_version():
    return jsonify({"installed": get_version()})


@system_bp.route("/api/ytdlp-update", methods=["POST"])
def api_ytdlp_update():
    def on_line(line):
        socketio.emit("update_log", {"line": line})

    def on_done(ok, message):
        logger.info("yt-dlp update finished, ok=%s", ok)
        socketio.emit("update_done", {"ok": ok, "message": message})

    update_async(on_line, on_done)
    return jsonify({"started": True})
