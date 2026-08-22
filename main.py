"""
ClipVault - punto de entrada.
Arma la app Flask, registra todas las rutas, arranca el JobManager y abre
la ventana de escritorio con pywebview.
"""
import sys
import os

if sys.stdin is None:
    sys.stdin = open(os.devnull, "r")
if sys.stdout is None:
    sys.stdout = open(os.devnull, "w")
if sys.stderr is None:
    sys.stderr = open(os.devnull, "w")

import threading
import traceback

import webview
from flask import Flask, render_template

from core.extensions import socketio
from core.logger import logger
from core.taskbar import find_window_by_title, set_window_icon, set_app_user_model_id, resource_path
from downloader.job_manager import manager

from routes.extract_routes import extract_bp
from routes.queue_routes import queue_bp
from routes.profile_routes import profile_bp
from routes.settings_routes import settings_bp
from routes.history_routes import history_bp
from routes.system_routes import system_bp
from routes.extension_routes import extension_bp


def create_app():
    app = Flask(__name__)
    app.config["SECRET_KEY"] = "dev-secret-change-me"
    socketio.init_app(app)

    app.register_blueprint(extract_bp)
    app.register_blueprint(queue_bp)
    app.register_blueprint(profile_bp)
    app.register_blueprint(settings_bp)
    app.register_blueprint(history_bp)
    app.register_blueprint(system_bp)
    app.register_blueprint(extension_bp)

    @app.route("/")
    def index():
        return render_template("index.html")

    return app


app = create_app()
manager.start_dispatcher()


def start_server():
    try:
        logger.info("Iniciando servidor Flask/SocketIO en 127.0.0.1:5000...")
        socketio.run(app, host="127.0.0.1", port=5000, debug=False, use_reloader=False, allow_unsafe_werkzeug=True)
    except Exception:
        logger.error("EL SERVIDOR FLASK FALLO AL ARRANCAR:\n%s", traceback.format_exc())


def on_window_ready():
    find_window_by_title("ClipVault")
    icon_path = resource_path(os.path.join("static", "icon.ico"))
    set_window_icon(icon_path)


if __name__ == "__main__":
    set_app_user_model_id("ClipVault.App.1")
    threading.Thread(target=start_server, daemon=True).start()
    webview.create_window("ClipVault", "http://127.0.0.1:5000", width=1150, height=820, min_size=(800, 600))
    webview.start(on_window_ready)
