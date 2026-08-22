"/api/download y /api/queue/* -- encolar videos y controlar pause/resume/cancel."
import os
import shutil
import uuid

from flask import Blueprint, request, jsonify

from core.config import get_settings, get_profiles, DEFAULT_DOWNLOAD_DIR
from core.utils import build_job_download_dir
from core.logger import logger
from downloader.job_manager import manager

queue_bp = Blueprint("queue", __name__)


@queue_bp.route("/api/download", methods=["POST"])
def api_download():
    body = request.json
    items = body.get("items", [])
    profile_id = body.get("profile_id")
    playlist_title = body.get("playlist_title") or ""
    if not items:
        return jsonify({"error": "No hay videos seleccionados"}), 400
    profile = next((p for p in get_profiles() if p["id"] == profile_id), None)
    if not profile:
        return jsonify({"error": "Perfil no encontrado"}), 400

    base_settings = get_settings()
    base_dir = base_settings.get("download_dir") or DEFAULT_DOWNLOAD_DIR
    dest_dir = build_job_download_dir(base_dir, profile["name"], playlist_title)
    os.makedirs(dest_dir, exist_ok=True)
    try:
        free_gb = round(shutil.disk_usage(dest_dir).free / (1024 ** 3), 2)
    except Exception:
        free_gb = None

    job_settings = dict(base_settings)
    job_settings["download_dir"] = dest_dir

    job_id = str(uuid.uuid4())[:8]
    manager.register_job(job_id, len(items))
    item_ids = [manager.enqueue(v, profile, job_settings, job_id) for v in items]
    logger.info("Job %s creado con %s elementos (perfil=%s, carpeta=%s)", job_id, len(items), profile["name"], dest_dir)
    return jsonify({"job_id": job_id, "total": len(items), "item_ids": item_ids, "free_disk_gb": free_gb, "dest_dir": dest_dir})


@queue_bp.route("/api/queue")
def api_queue():
    return jsonify({"items": manager.snapshot()})


@queue_bp.route("/api/queue/clear", methods=["POST"])
def api_queue_clear():
    cleared = manager.clear_active()
    logger.info("Cola activa limpiada (%s elementos)", cleared)
    return jsonify({"ok": True, "cleared": cleared})


@queue_bp.route("/api/queue/<item_id>/pause", methods=["POST"])
def api_pause(item_id):
    manager.pause(item_id)
    return jsonify({"ok": True})


@queue_bp.route("/api/queue/<item_id>/resume", methods=["POST"])
def api_resume(item_id):
    manager.resume(item_id)
    return jsonify({"ok": True})


@queue_bp.route("/api/queue/<item_id>/cancel", methods=["POST"])
def api_cancel(item_id):
    manager.cancel(item_id)
    return jsonify({"ok": True})


@queue_bp.route("/api/queue/reorder-full", methods=["POST"])
def api_reorder_full():
    order = (request.json or {}).get("order", [])
    manager.set_full_order(order)
    return jsonify({"ok": True})


@queue_bp.route("/api/disk-space")
def api_disk_space():
    settings = get_settings()
    dest_dir = settings.get("download_dir") or DEFAULT_DOWNLOAD_DIR
    try:
        usage = shutil.disk_usage(dest_dir)
        return jsonify({"free_gb": round(usage.free / (1024 ** 3), 2), "total_gb": round(usage.total / (1024 ** 3), 2)})
    except Exception as exc:
        return jsonify({"error": str(exc)}), 500
