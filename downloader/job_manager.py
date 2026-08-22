"""El corazon de ClipVault: la cola de descargas.
Migrado a subprocess + yt-dlp.exe (en vez de la libreria yt_dlp) para que
yt-dlp pueda actualizarse solo con 'yt-dlp.exe -U' sin recompilar la app.
Incluye todos los arreglos de estabilidad previos:
- El dispatcher nunca relanza un item_id que ya esta activo.
- cancel() usa un estado transitorio "cancelling" mientras el proceso real
  confirma la parada (evita residuales al vaciar la cola).
- resume() no reencola un item que ya sigue activo.
- El progreso se throttlea a ~2 emisiones/segundo.
- Al cancelar de verdad, el item se borra de la cola activa
  (evento "item_removed") y pasa al historial.
- Progreso reflejado en el icono de la barra de tareas de Windows.
"""
import os
import time
import uuid
import shutil
import subprocess
import threading
import itertools
from datetime import datetime

from core.config import get_settings, DEFAULT_DOWNLOAD_DIR, MIN_FREE_BYTES
from core.logger import logger
from core.utils import sanitize_filename, is_permanent_error, CancelledError, notify_desktop
from core.history import append_history
from core.registry import load_registry, mark_downloaded
from core.extensions import socketio
from core.taskbar import set_progress
from downloader.ydl_options import profile_to_ytdlp_opts, common_ydl_opts, advanced_ydl_opts, merge_postprocessors
from downloader.cli_args import opts_to_cli_args
from downloader.ytdlp_bin import ytdlp_path


class JobManager:
    def __init__(self):
        self.lock = threading.RLock()
        self.items = {}
        self.pending = set()
        self.active = set()
        self.cancel_events = {}
        self.procs = {}
        self.job_totals = {}
        self.job_done = {}
        self._seq_counter = itertools.count()
        self._dispatcher_started = False

    def start_dispatcher(self):
        with self.lock:
            if self._dispatcher_started:
                return
            self._dispatcher_started = True
            threading.Thread(target=self._dispatch_loop, daemon=True).start()

    def register_job(self, job_id, total):
        with self.lock:
            self.job_totals[job_id] = self.job_totals.get(job_id, 0) + total
            self.job_done.setdefault(job_id, 0)

    def _job_progress(self, job_id):
        with self.lock:
            self.job_done[job_id] = self.job_done.get(job_id, 0) + 1
            done, total = self.job_done[job_id], self.job_totals.get(job_id, 0)
        if total and done >= total:
            notify_desktop("Descarga completa", f"Se terminaron {total} elemento(s) del job {job_id}.")
            logger.info("Job %s completado (%s elementos)", job_id, total)
            socketio.emit("job_finished", {"job_id": job_id, "total": total})

    def enqueue(self, video_item, profile, settings, job_id):
        item_id = str(uuid.uuid4())[:10]
        dest_dir = settings.get("download_dir") or DEFAULT_DOWNLOAD_DIR
        registry = load_registry(dest_dir)
        state = "queued"
        if video_item.get("id") in registry:
            state = "skipped"
        seq = next(self._seq_counter)
        with self.lock:
            self.items[item_id] = {
                "item_id": item_id, "job_id": job_id, "video": video_item,
                "profile": profile, "settings": settings, "status": state,
                "pct": 0, "error": None, "paused": False, "seq": seq,
            }
            if state == "queued":
                self.pending.add(item_id)
        self._emit_snapshot(item_id)
        if state == "skipped":
            self._log_history(item_id)
            self._job_progress(job_id)
        return item_id

    def _emit_snapshot(self, item_id):
        it = self.items.get(item_id)
        if not it:
            return
        socketio.emit("item_status", {
            "item_id": item_id, "job_id": it["job_id"], "title": it["video"]["title"],
            "status": it["status"], "pct": it["pct"], "error": it["error"], "order": it["seq"],
            "thumbnail": it["video"].get("thumbnail"), "url": it["video"].get("url"),
            "video_id": it["video"].get("id"), "extractor": it["video"].get("extractor"),
        })
        self._update_taskbar()

    def _update_taskbar(self):
        with self.lock:
            items = list(self.items.values())
        if not items:
            set_progress(None)
            return
        total = len(items)
        accum = 0
        for it in items:
            if it["status"] == "done":
                accum += 100
            elif it["status"] == "downloading":
                accum += it["pct"]
        set_progress(accum / total)

    def _log_history(self, item_id):
        it = self.items.get(item_id)
        if not it:
            return
        append_history({
            "item_id": item_id, "job_id": it["job_id"], "title": it["video"]["title"],
            "url": it["video"]["url"], "video": it["video"],
            "profile_id": it["profile"]["id"], "profile_name": it["profile"]["name"],
            "profile": it["profile"], "status": it["status"], "error": it["error"],
            "dest_dir": it["settings"].get("download_dir") or DEFAULT_DOWNLOAD_DIR,
            "finished_at": datetime.now().isoformat(timespec="seconds"),
        })
        socketio.emit("history_updated", {})

    def set_full_order(self, ordered_ids):
        with self.lock:
            for idx, iid in enumerate(ordered_ids):
                if iid in self.items:
                    self.items[iid]["seq"] = idx
        self._broadcast_order()

    def _broadcast_order(self):
        with self.lock:
            snapshot = [(iid, it["seq"]) for iid, it in self.items.items()]
        for iid, seq in snapshot:
            socketio.emit("item_order", {"item_id": iid, "order": seq})

    def pause(self, item_id):
        with self.lock:
            it = self.items.get(item_id)
            if not it:
                return
            it["paused"] = True
            if item_id in self.active and item_id in self.cancel_events:
                self.cancel_events[item_id].set()
                self._kill_proc(item_id)
            it["status"] = "paused"
        self._emit_snapshot(item_id)

    def resume(self, item_id):
        with self.lock:
            it = self.items.get(item_id)
            if not it:
                return
            it["paused"] = False
            if item_id in self.active:
                return
            it["status"] = "queued"
            self.pending.add(item_id)
        self._emit_snapshot(item_id)

    def cancel(self, item_id):
        with self.lock:
            it = self.items.get(item_id)
            if not it:
                return
            if item_id in self.cancel_events:
                self.cancel_events[item_id].set()
                self._kill_proc(item_id)
                it["status"] = "cancelling"
                self._emit_snapshot(item_id)
                return
            it["status"] = "cancelled"
            self.pending.discard(item_id)
        self._log_history(item_id)
        self._job_progress(it["job_id"])
        with self.lock:
            self.items.pop(item_id, None)
        socketio.emit("item_removed", {"item_id": item_id})
        self._update_taskbar()

    def _kill_proc(self, item_id):
        proc = self.procs.get(item_id)
        if proc and proc.poll() is None:
            try:
                proc.terminate()
            except Exception:
                pass

    def clear_active(self):
        """Quita de la cola activa todo lo que ya termino: done/cancelled/failed/skipped."""
        with self.lock:
            to_remove = [iid for iid, it in self.items.items()
                         if it["status"] in ("done", "cancelled", "failed", "failed (no recuperable)", "skipped")]
            for iid in to_remove:
                self.items.pop(iid, None)
                self.pending.discard(iid)
                self.active.discard(iid)
                self.cancel_events.pop(iid, None)
                self.procs.pop(iid, None)
        socketio.emit("queue_cleared", {"cleared": len(to_remove)})
        self._update_taskbar()
        return len(to_remove)

    def snapshot(self):
        with self.lock:
            return [
                {"item_id": iid, "job_id": it["job_id"], "title": it["video"]["title"],
                 "status": it["status"], "pct": it["pct"], "error": it["error"], "order": it["seq"],
                 "thumbnail": it["video"].get("thumbnail"), "url": it["video"].get("url"),
                 "video_id": it["video"].get("id"), "extractor": it["video"].get("extractor")}
                for iid, it in self.items.items()
            ]

    def _dispatch_loop(self):
        while True:
            with self.lock:
                settings = get_settings()
                concurrency = max(1, int(settings.get("concurrency", 2)))
                free_slots = concurrency - len(self.active)
                candidates = []
                if free_slots > 0:
                    ready = [
                        iid for iid in self.pending
                        if iid in self.items
                        and not self.items[iid]["paused"]
                        and iid not in self.active
                    ]
                    ready.sort(key=lambda iid: self.items[iid]["seq"])
                    candidates = ready[:free_slots]
                for iid in candidates:
                    self.pending.discard(iid)
                    self.active.add(iid)
                    self.cancel_events[iid] = threading.Event()
                    threading.Thread(target=self._run_item, args=(iid,), daemon=True).start()
            time.sleep(0.4)

    def _run_item(self, item_id):
        it = self.items[item_id]
        it["status"] = "downloading"
        self._emit_snapshot(item_id)
        video, profile, settings = it["video"], it["profile"], it["settings"]
        dest_dir = settings.get("download_dir") or DEFAULT_DOWNLOAD_DIR
        os.makedirs(dest_dir, exist_ok=True)

        try:
            usage = shutil.disk_usage(dest_dir)
            if usage.free < MIN_FREE_BYTES:
                it["status"] = "failed"
                it["error"] = "Espacio en disco insuficiente"
                self._emit_snapshot(item_id)
                with self.lock:
                    self.active.discard(item_id)
                    self.cancel_events.pop(item_id, None)
                self._log_history(item_id)
                self._job_progress(it["job_id"])
                logger.warning("Fallo por espacio insuficiente: %s", video.get("title"))
                return
        except Exception:
            pass

        cancel_event = self.cancel_events[item_id]
        last_emit = {"pct": -1, "time": 0}

        def emit_progress(pct):
            now = time.time()
            if pct != last_emit["pct"] and now - last_emit["time"] >= 0.4:
                it["pct"] = pct
                last_emit["pct"] = pct
                last_emit["time"] = now
                socketio.emit("item_status", {
                    "item_id": item_id, "job_id": it["job_id"], "title": video["title"],
                    "status": "downloading", "pct": it["pct"], "error": None, "order": it["seq"],
                    "thumbnail": video.get("thumbnail"), "url": video.get("url"),
                    "video_id": video.get("id"), "extractor": video.get("extractor"),
                })
                self._update_taskbar()

        safe_title = sanitize_filename(video.get("title") or "video")
        out_template = os.path.join(dest_dir, f"{safe_title}.%(ext)s")

        opts = {"retries": 3, "continuedl": True}
        opts.update(common_ydl_opts(settings))
        opts = merge_postprocessors(opts, profile_to_ytdlp_opts(profile))
        opts = merge_postprocessors(opts, advanced_ydl_opts(settings))

        cautious = settings.get("cautious_mode", "balanced")
        if cautious == "balanced":
            opts["sleep_interval"], opts["max_sleep_interval"] = 1, 3
        elif cautious == "careful":
            opts["sleep_interval"], opts["max_sleep_interval"] = 3, 8

        cli_args = opts_to_cli_args(opts)

        last_error, success = None, False
        for attempt in range(1, 4):
            if cancel_event.is_set():
                break
            try:
                proc = subprocess.Popen(
                    [ytdlp_path(), "-o", out_template, "--newline", *cli_args, video["url"]],
                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, creationflags=subprocess.CREATE_NO_WINDOW
                )
                with self.lock:
                    self.procs[item_id] = proc

                output_lines = []
                for line in proc.stdout:
                    output_lines.append(line)
                    if cancel_event.is_set():
                        proc.terminate()
                        break
                    if "[download]" in line and "%" in line:
                        try:
                            pct_str = line.split("%")[0].strip().split()[-1]
                            emit_progress(round(float(pct_str), 1))
                        except Exception:
                            pass
                proc.wait()

                if cancel_event.is_set():
                    last_error = "cancelled"
                    break
                if proc.returncode == 0:
                    success = True
                    break
                last_error = "".join(output_lines[-15:]).strip() or f"yt-dlp termino con codigo {proc.returncode}"
                if is_permanent_error(last_error):
                    break
            except FileNotFoundError:
                last_error = "No se encontro yt-dlp.exe junto al programa."
                break
            except Exception as exc:
                last_error = str(exc)
                if is_permanent_error(last_error):
                    break
            time.sleep(1.5 * attempt)

        with self.lock:
            self.active.discard(item_id)
            self.cancel_events.pop(item_id, None)
            self.procs.pop(item_id, None)

        job_id = it["job_id"]
        if success:
            it["status"], it["pct"] = "done", 100
            if video.get("id"):
                mark_downloaded(dest_dir, video["id"])
            logger.info("Descarga OK: %s", video.get("title"))
            self._emit_snapshot(item_id)
            self._log_history(item_id)
            self._job_progress(job_id)
        elif last_error == "cancelled":
            if it["paused"]:
                it["status"] = "paused"
                self._emit_snapshot(item_id)
            else:
                it["status"] = "cancelled"
                self._log_history(item_id)
                self._job_progress(job_id)
                with self.lock:
                    self.items.pop(item_id, None)
                    self.pending.discard(item_id)
                socketio.emit("item_removed", {"item_id": item_id})
                self._update_taskbar()
        else:
            it["status"] = "failed (no recuperable)" if is_permanent_error(last_error or "") else "failed"
            it["error"] = last_error
            logger.error("Descarga fallida: %s -- %s", video.get("title"), last_error)
            self._emit_snapshot(item_id)
            self._log_history(item_id)
            self._job_progress(job_id)


manager = JobManager()
