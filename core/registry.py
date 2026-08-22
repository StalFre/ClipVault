"Registro de IDs de video ya descargados por carpeta de destino (.downloaded_ids.json), para evitar re-descargar lo mismo en la misma carpeta."
import os
import json


def registry_path(dest_dir):
    return os.path.join(dest_dir, ".downloaded_ids.json")


def load_registry(dest_dir):
    p = registry_path(dest_dir)
    if os.path.exists(p):
        try:
            with open(p, "r", encoding="utf-8") as f:
                return set(json.load(f))
        except Exception:
            return set()
    return set()


def mark_downloaded(dest_dir, video_id):
    ids = load_registry(dest_dir)
    ids.add(video_id)
    with open(registry_path(dest_dir), "w", encoding="utf-8") as f:
        json.dump(list(ids), f)
