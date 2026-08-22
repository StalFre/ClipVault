"/api/settings, /api/browse-folder, /api/open-folder -- configuracion y carpetas."
import os
import platform
import subprocess

from flask import Blueprint, request, jsonify

from core.config import get_settings, save_settings, DEFAULT_DOWNLOAD_DIR

settings_bp = Blueprint("settings", __name__)


@settings_bp.route("/api/settings", methods=["GET"])
def api_get_settings():
    return jsonify(get_settings())


@settings_bp.route("/api/settings", methods=["POST"])
def api_save_settings():
    settings = get_settings()
    settings.update(request.json or {})
    save_settings(settings)
    return jsonify({"ok": True, "settings": settings})


@settings_bp.route("/api/browse-folder", methods=["POST"])
def api_browse_folder():
    try:
        import tkinter as tk
        from tkinter import filedialog
        root = tk.Tk()
        root.withdraw()
        root.attributes("-topmost", True)
        folder = filedialog.askdirectory(title="Selecciona la carpeta de destino")
        root.destroy()
        return jsonify({"path": folder}) if folder else jsonify({"cancelled": True})
    except Exception as exc:
        return jsonify({"error": f"No se pudo abrir el selector nativo: {exc}"}), 500


@settings_bp.route("/api/open-folder", methods=["POST"])
def api_open_folder():
    path = (request.json or {}).get("path") or DEFAULT_DOWNLOAD_DIR
    if not os.path.isdir(path):
        return jsonify({"error": "La carpeta no existe"}), 400
    try:
        system = platform.system()
        if system == "Windows":
            os.startfile(path)  # noqa: S606
        elif system == "Darwin":
            subprocess.Popen(["open", path])
        else:
            subprocess.Popen(["xdg-open", path])
        return jsonify({"ok": True})
    except Exception as exc:
        return jsonify({"error": str(exc)}), 500


@settings_bp.route("/api/open-url", methods=["POST"])
def api_open_url():
    import webbrowser
    url = (request.json or {}).get("url", "")
    if not url.startswith("http"):
        return jsonify({"error": "URL invalida"}), 400
    try:
        webbrowser.open(url)
        return jsonify({"ok": True})
    except Exception as exc:
        return jsonify({"error": str(exc)}), 500
