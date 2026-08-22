"/api/extract, /api/search, /api/extract-bulk -- extraer metadata sin descargar."
from flask import Blueprint, request, jsonify

from core.config import get_settings
from core.logger import logger
from downloader.extractor import extract_info

extract_bp = Blueprint("extract", __name__)


@extract_bp.route("/api/extract", methods=["POST"])
def api_extract():
    body = request.json
    url = (body.get("url") or "").strip()
    single = bool(body.get("single", False))
    if not url:
        return jsonify({"error": "URL vacia"}), 400
    try:
        data = extract_info(url, get_settings(), single)
        return jsonify(data)
    except Exception as exc:
        logger.error("Fallo al extraer %s: %s", url, exc)
        return jsonify({"error": str(exc)}), 500


@extract_bp.route("/api/search", methods=["POST"])
def api_search():
    body = request.json or {}
    query = (body.get("query") or "").strip()
    max_results = int(body.get("max_results", 15))
    if not query:
        return jsonify({"error": "Escribe algo para buscar"}), 400
    try:
        data = extract_info(f"ytsearch{max_results}:{query}", get_settings(), single=False)
        return jsonify(data)
    except Exception as exc:
        logger.error("Fallo al buscar '%s': %s", query, exc)
        return jsonify({"error": str(exc)}), 500


@extract_bp.route("/api/extract-bulk", methods=["POST"])
def api_extract_bulk():
    urls = [u.strip() for u in (request.json.get("urls") or []) if u.strip()]
    settings = get_settings()
    items, failed, skipped = [], [], []
    for u in urls:
        try:
            data = extract_info(u, settings, single=True)
            items.extend(data["items"])
            skipped.extend(data.get("skipped", []))
        except Exception as exc:
            failed.append({"url": u, "error": str(exc)})
            skipped.append({"title": "No se pudo cargar", "reason": str(exc), "url": u})
    return jsonify({"items": items, "failed": failed, "skipped": skipped, "total_count": len(urls)})
