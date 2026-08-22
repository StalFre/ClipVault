"""Wrapper para usar el binario standalone yt-dlp.exe via subprocess,
en vez de la libreria de Python empaquetada dentro del .exe de la app.

Esto permite que yt-dlp se actualice solo (yt-dlp.exe -U) sin tener que
volver a compilar ni reinstalar ClipVault. yt-dlp.exe debe vivir junto
al ejecutable principal (misma carpeta de instalacion).
"""
import os
import sys
import json
import subprocess
import threading

_NO_WINDOW = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0

from core.logger import logger


def _app_dir():
    """Carpeta donde vive el .exe (o el script, en desarrollo)."""
    if getattr(sys, "frozen", False):
        return os.path.dirname(sys.executable)
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def ytdlp_path():
    """Ruta esperada de yt-dlp.exe, junto al ejecutable principal."""
    candidate = os.path.join(_app_dir(), "yt-dlp.exe")
    if os.path.isfile(candidate):
        return candidate
    # Fallback para desarrollo: usa el yt-dlp del PATH si existe.
    return "yt-dlp"


def get_version():
    try:
        result = subprocess.run(
            [ytdlp_path(), "--version"],
            capture_output=True, text=True, timeout=15,  creationflags=_NO_WINDOW
        )
        return result.stdout.strip() or "desconocida"
    except Exception as exc:
        logger.warning("No se pudo leer la version de yt-dlp: %s", exc)
        return "desconocida"


def update_async(on_line, on_done):
    """Corre 'yt-dlp.exe -U' en un hilo aparte.
    on_line(str) se llama por cada linea de salida.
    on_done(bool ok, str message) se llama al terminar."""

    def _run():
        try:
            proc = subprocess.Popen(
                [ytdlp_path(), "-U"],
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, creationflags=_NO_WINDOW
            )
            for line in proc.stdout:
                on_line(line.rstrip())
            proc.wait()
            ok = proc.returncode == 0
            logger.info("yt-dlp -U finalizado, ok=%s", ok)
            on_done(ok, "yt-dlp actualizado correctamente." if ok else f"yt-dlp termino con codigo {proc.returncode}")
        except FileNotFoundError:
            msg = "No se encontro yt-dlp.exe junto al programa. Reinstala o colocalo manualmente."
            logger.warning(msg)
            on_done(False, msg)
        except Exception as exc:
            logger.warning("Error actualizando yt-dlp: %s", exc)
            on_done(False, str(exc))

    threading.Thread(target=_run, daemon=True).start()


def extract_info(url, extra_args=None):
    """Extrae metadata (sin descargar) usando --dump-single-json."""
    args = [ytdlp_path(), "--dump-single-json", "--no-warnings", "--flat-playlist", url]
    if extra_args:
        args.extend(extra_args)
    result = subprocess.run(
        args, capture_output=True, text=True, timeout=120,
        creationflags=_NO_WINDOW
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "yt-dlp fallo al extraer informacion")
    return json.loads(result.stdout)


def download(url, out_template, extra_args, progress_callback=None):
    """Descarga un video/audio. extra_args: lista de flags de yt-dlp
    (formato, postprocesadores, etc). progress_callback(pct: float|None) opcional."""
    args = [ytdlp_path(), "-o", out_template, "--newline", *extra_args, url]
    proc = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,  creationflags=_NO_WINDOW)
    for line in proc.stdout:
        if progress_callback and "[download]" in line and "%" in line:
            try:
                pct_str = line.split("%")[0].split()[-1]
                progress_callback(float(pct_str))
            except Exception:
                pass
    proc.wait()
    return proc.returncode == 0
