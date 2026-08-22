"/api/history/* -- historial de descargas y reintentos (sin duplicar registros)."
import uuid

from flask import Blueprint, request, jsonify

from core.config import get_settings
from core.history import load_history, save_history
from core.extensions import socketio
from downloader.job_manager import manager

history_bp = Blueprint("history", __name__)


@history_bp.route("/api/history")
def api_history():
    q = (request.args.get("q") or "").lower().strip()
    status = request.args.get("status") or ""
    records = load_history()
    if q:
        records = [r for r in records if q in r.get("title", "").lower()]
    if status:
        records = [r for r in records if r.get("status") == status]
    records = sorted(records, key=lambda r: r.get("finished_at", ""), reverse=True)
    return jsonify({"records": records[:300]})


@history_bp.route("/api/history", methods=["DELETE"])
def api_history_clear():
    save_history([])
    socketio.emit("history_updated", {})
    return jsonify({"ok": True})


@history_bp.route("/api/history/<item_id>/retry", methods=["POST"])
def api_history_retry(item_id):
    records = load_history()
    record = next((r for r in records if r["item_id"] == item_id), None)
    if not record:
        return jsonify({"error": "Registro no encontrado"}), 404

    # Se borra el registro viejo antes de reintentar para no acumular
    # varias filas del mismo video en el historial.
    records = [r for r in records if r["item_id"] != item_id]
    save_history(records)
    socketio.emit("history_updated", {})

    settings = get_settings()
    settings["download_dir"] = record.get("dest_dir") or settings.get("download_dir")
    job_id = str(uuid.uuid4())[:8]
    manager.register_job(job_id, 1)
    new_item_id = manager.enqueue(record["video"], record["profile"], settings, job_id)
    return jsonify({"ok": True, "item_id": new_item_id, "job_id": job_id})
