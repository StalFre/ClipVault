"/api/profiles/* -- perfiles de calidad (built-in y personalizados)."
import uuid

from flask import Blueprint, request, jsonify

from core.config import get_settings, save_settings, get_profiles, save_profiles, DEFAULT_PROFILES

profile_bp = Blueprint("profile", __name__)


@profile_bp.route("/api/profiles", methods=["GET"])
def api_get_profiles():
    return jsonify({"profiles": get_profiles(), "active_profile_id": get_settings().get("active_profile_id")})


@profile_bp.route("/api/profiles", methods=["POST"])
def api_upsert_profile():
    body = request.json
    profiles = get_profiles()
    pid = body.get("id") or str(uuid.uuid4())[:8]
    existing = next((p for p in profiles if p["id"] == pid), None)
    kind = body.get("kind", "video")
    new_profile = {
        "id": pid, "name": body.get("name", "Perfil personalizado"),
        "builtin": existing["builtin"] if existing else False,
        "kind": kind, "height": body.get("height"),
        "container": body.get("container", "native"),
        "audio_codec": body.get("audio_codec", "mp3"),
        "desc": body.get("desc", ""),
    }
    profiles = [new_profile if p["id"] == pid else p for p in profiles] if existing else profiles + [new_profile]
    save_profiles(profiles)
    return jsonify({"ok": True, "profile": new_profile})


@profile_bp.route("/api/profiles/<pid>/reset", methods=["POST"])
def api_reset_profile(pid):
    default = next((p for p in DEFAULT_PROFILES if p["id"] == pid), None)
    if not default:
        return jsonify({"error": "No es un perfil integrado"}), 400
    profiles = [default.copy() if p["id"] == pid else p for p in get_profiles()]
    save_profiles(profiles)
    return jsonify({"ok": True, "profile": default})


@profile_bp.route("/api/profiles/<pid>", methods=["DELETE"])
def api_delete_profile(pid):
    profiles = [p for p in get_profiles() if not (p["id"] == pid and not p.get("builtin"))]
    save_profiles(profiles)
    return jsonify({"ok": True})


@profile_bp.route("/api/profiles/active", methods=["POST"])
def api_set_active_profile():
    settings = get_settings()
    settings["active_profile_id"] = request.json.get("profile_id")
    save_settings(settings)
    return jsonify({"ok": True})
