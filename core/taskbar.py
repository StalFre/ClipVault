"""Integracion con la barra de tareas de Windows:
- Progreso en el icono (ITaskbarList3 via comtypes).
- Parpadeo del icono para avisar cuando llega un enlace de la extension.
- Cambio del icono de la ventana (titulo + barra de tareas).

Cada funcionalidad se activa por separado: si comtypes falla, el progreso
se desactiva solo, pero el parpadeo y el icono (que solo usan pywin32)
siguen funcionando normal.
"""
import os
import sys
import time
import threading

from core.logger import logger

_hwnd = None
_lock = threading.Lock()
_is_windows = sys.platform == "win32"

PYWIN32_OK = False
if _is_windows:
    try:
        import win32gui
        import win32con
        import win32api
        PYWIN32_OK = True
    except Exception as exc:
        logger.warning("pywin32 no disponible, funciones de ventana nativa desactivadas: %s", exc)

COMTYPES_OK = False
_taskbar = None
if _is_windows:
    try:
        import comtypes
        import comtypes.client
        from comtypes import GUID, COMMETHOD, HRESULT
        from ctypes import c_int, c_ulonglong
        from ctypes.wintypes import HWND

        CLSID_TaskbarList = GUID("{56FDF344-FD6D-11d0-958A-006097C9A090}")

        class ITaskbarList3(comtypes.IUnknown):
            _iid_ = GUID("{ea1afb91-9e28-4b86-90e9-9e9f8a5eefaf}")
            _methods_ = [
                COMMETHOD([], HRESULT, "HrInit"),
                COMMETHOD([], HRESULT, "AddTab", (["in"], HWND, "hwnd")),
                COMMETHOD([], HRESULT, "DeleteTab", (["in"], HWND, "hwnd")),
                COMMETHOD([], HRESULT, "ActivateTab", (["in"], HWND, "hwnd")),
                COMMETHOD([], HRESULT, "SetActiveAlt", (["in"], HWND, "hwnd")),
                COMMETHOD([], HRESULT, "MarkFullscreenWindow", (["in"], HWND, "hwnd"), (["in"], c_int, "on")),
                COMMETHOD([], HRESULT, "SetProgressValue", (["in"], HWND, "hwnd"),
                          (["in"], c_ulonglong, "completed"), (["in"], c_ulonglong, "total")),
                COMMETHOD([], HRESULT, "SetProgressState", (["in"], HWND, "hwnd"), (["in"], c_int, "state")),
            ]

        TBPF_NOPROGRESS = 0
        TBPF_NORMAL = 0x2
        TBPF_ERROR = 0x4
        COMTYPES_OK = True
    except Exception as exc:
        logger.warning("comtypes no disponible, progreso en icono desactivado: %s", exc)


def set_app_user_model_id(app_id="ClipVault.App.1"):
    """Le da a la app su propia identidad ante Windows (AppUserModelID).

    Sin esto, cuando se corre con 'python main.py' (no empaquetado), Windows
    agrupa el icono de la barra de tareas bajo python.exe y el icono
    personalizado no se refleja ahi, aunque si cambie en la barra de titulo.
    Al empaquetar con PyInstaller esto deja de ser necesario porque el .exe
    ya tiene su propia identidad, pero no hace daño dejarlo.
    """
    if not _is_windows:
        return
    try:
        from ctypes import windll
        windll.shell32.SetCurrentProcessExplicitAppUserModelID(app_id)
        logger.info("AppUserModelID establecido: %s", app_id)
    except Exception as exc:
        logger.info("No se pudo establecer AppUserModelID (no critico): %s", exc)


def resource_path(relative_path):
    """Resuelve rutas de recursos tanto en desarrollo como empaquetado
    con PyInstaller (--onefile extrae los datos a sys._MEIPASS)."""
    base_path = getattr(sys, "_MEIPASS", os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    return os.path.join(base_path, relative_path)


def find_window_by_title(title="ClipVault", retries=20, delay=0.5):
    """Busca el HWND de la ventana por titulo, reintentando varias veces
    porque la ventana puede tardar un poco en terminar de crearse."""
    global _hwnd
    if not PYWIN32_OK:
        return
    for attempt in range(retries):
        try:
            hwnd = win32gui.FindWindow(None, title)
            if hwnd:
                _hwnd = hwnd
                logger.info("Ventana '%s' encontrada tras %d intento(s), hwnd=%s", title, attempt + 1, hwnd)
                return
        except Exception as exc:
            logger.warning("Error buscando la ventana: %s", exc)
            return
        time.sleep(delay)
    logger.warning("No se encontro ninguna ventana con titulo '%s' tras %d intentos", title, retries)


def _get_taskbar():
    global _taskbar
    if _taskbar is None and COMTYPES_OK:
        try:
            _taskbar = comtypes.client.CreateObject(CLSID_TaskbarList, interface=ITaskbarList3)
            _taskbar.HrInit()
        except Exception as exc:
            logger.warning("No se pudo crear ITaskbarList3: %s", exc)
            return None
    return _taskbar


def set_progress(percent):
    """percent: 0-100, o None para quitar el indicador."""
    if not COMTYPES_OK or _hwnd is None:
        return
    tb = _get_taskbar()
    if not tb:
        return
    with _lock:
        try:
            if percent is None:
                tb.SetProgressState(_hwnd, TBPF_NOPROGRESS)
            else:
                tb.SetProgressState(_hwnd, TBPF_NORMAL)
                tb.SetProgressValue(_hwnd, int(percent), 100)
        except Exception as exc:
            logger.warning("Error actualizando progreso en taskbar: %s", exc)


def set_error_state():
    if not COMTYPES_OK or _hwnd is None:
        return
    tb = _get_taskbar()
    if tb:
        try:
            tb.SetProgressState(_hwnd, TBPF_ERROR)
        except Exception:
            pass


def focus_window():
    if not PYWIN32_OK or _hwnd is None:
        return
    try:
        win32gui.ShowWindow(_hwnd, win32con.SW_RESTORE)
        win32gui.SetForegroundWindow(_hwnd)
    except Exception as exc:
        logger.info("No se pudo forzar el foco (normal por restricciones de Windows): %s", exc)


def flash_window():
    """Hace parpadear el icono en la barra de tareas hasta que el usuario
    haga clic en la ventana. Solo depende de pywin32, no de comtypes."""
    if not PYWIN32_OK or _hwnd is None:
        logger.warning("flash_window: no disponible (PYWIN32_OK=%s, hwnd=%s)", PYWIN32_OK, _hwnd)
        return
    try:
        from ctypes import Structure, sizeof, byref, windll
        from ctypes.wintypes import HWND, UINT, DWORD

        class FLASHWINFO(Structure):
            _fields_ = [
                ("cbSize", UINT),
                ("hwnd", HWND),
                ("dwFlags", DWORD),
                ("uCount", UINT),
                ("dwTimeout", DWORD),
            ]

        FLASHW_ALL = 0x00000003
        FLASHW_TIMERNOFG = 0x0000000C

        flash_info = FLASHWINFO(sizeof(FLASHWINFO), _hwnd, FLASHW_ALL | FLASHW_TIMERNOFG, 0, 0)
        result = windll.user32.FlashWindowEx(byref(flash_info))
        logger.info("FlashWindowEx llamado, resultado=%s", result)
    except Exception as exc:
        logger.warning("Error al hacer parpadear la ventana: %s", exc)


def set_window_icon(icon_path):
    """Cambia el icono de la ventana (barra de titulo y barra de tareas) desde un .ico."""
    logger.info("set_window_icon llamado con icon_path=%s", icon_path)

    if not PYWIN32_OK:
        logger.warning("set_window_icon: PYWIN32_OK es False, no se puede continuar")
        return
    if _hwnd is None:
        logger.warning("set_window_icon: hwnd es None, la ventana no se encontro todavia")
        return
    if not os.path.isfile(icon_path):
        logger.warning("set_window_icon: el archivo no existe en la ruta: %s", icon_path)
        return

    try:
        hicon = win32gui.LoadImage(
            0, icon_path, win32con.IMAGE_ICON, 0, 0,
            win32con.LR_LOADFROMFILE | win32con.LR_DEFAULTSIZE
        )
        if not hicon:
            logger.warning("set_window_icon: LoadImage devolvio 0 (el .ico puede estar mal formado): %s", icon_path)
            return

        win32api.SendMessage(_hwnd, win32con.WM_SETICON, win32con.ICON_SMALL, hicon)
        win32api.SendMessage(_hwnd, win32con.WM_SETICON, win32con.ICON_BIG, hicon)

        try:
            from ctypes import windll
            GCL_HICON = -14
            set_class_fn = getattr(windll.user32, "SetClassLongPtrW", None) or windll.user32.SetClassLongW
            set_class_fn(_hwnd, GCL_HICON, hicon)
            logger.info("Icono de clase de ventana fijado correctamente")
        except Exception as exc:
            logger.info("No se pudo fijar el icono de clase (no critico): %s", exc)

        logger.info("Icono de ventana aplicado correctamente desde %s", icon_path)
    except Exception as exc:
        logger.warning("Error cambiando el icono de la ventana: %s", exc)
