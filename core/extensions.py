"""Instancia unica de SocketIO, compartida por app.py, routes/ y downloader/.
Se inicializa (init_app) desde app.py para evitar imports circulares.
"""
from flask_socketio import SocketIO

socketio = SocketIO(cors_allowed_origins="*", async_mode="threading")
