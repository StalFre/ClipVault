"Persistencia del historial de descargas (history.json)."
import os
import threading

from core.config import BASE_DIR, load_json, save_json

HISTORY_PATH = os.path.join(BASE_DIR, "history.json")
history_lock = threading.Lock()


def load_history():
    with history_lock:
        return load_json(HISTORY_PATH, {"records": []})["records"] if os.path.exists(HISTORY_PATH) else []


def save_history(records):
    with history_lock:
        save_json(HISTORY_PATH, {"records": records})


def append_history(record):
    records = load_history()
    records.append(record)
    records = records[-2000:]
    save_history(records)
