"""Recibe enlaces enviados desde la extension de Chrome de ClipVault."""
from flask import Blueprint, request, jsonify

from core.extensions import socketio
from core.taskbar import focus_window, flash_window

extension_bp = Blueprint("extension", __name__)


@extension_bp.route("/api/extension/send", methods=["POST"])
def api_extension_send():
    body = request.json or {}
    url = (body.get("url") or "").strip()
    if not url:
        return jsonify({"error": "URL vacia"}), 400
    socketio.emit("extension_link", {"url": url})
    focus_window()
    flash_window()
    return jsonify({"ok": True})