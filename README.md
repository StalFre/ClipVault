# ClipVault

Gestor personal de descargas de video y audio, construido sobre [yt-dlp](https://github.com/yt-dlp/yt-dlp).

**Versión 1.0.0**

## Qué es ClipVault

ClipVault es una interfaz de escritorio propia sobre yt-dlp: extrae videos individuales, playlists completas o múltiples enlaces sueltos, permite elegir perfiles de calidad de video o audio, gestiona una cola de descargas con pausa/reanudar/cancelar y reordenamiento por arrastre, guarda un historial consultable, y corre como una ventana de escritorio nativa (no requiere abrir el navegador).

## Estructura del proyecto

```
ClipVault/
├── main.py                  # Punto de entrada: arma la app Flask y abre la ventana
├── core/                    # Configuración, logging, utilidades, historial, taskbar
├── downloader/               # Lógica de yt-dlp: opciones, extractor, cola de descargas
├── routes/                   # Endpoints Flask (extract, queue, profiles, settings, history...)
├── static/                   # CSS, JS, íconos
├── templates/                 # HTML
├── ClipVault-extension/       # Extensión de navegador complementaria
├── build.bat                 # Compila el .exe con PyInstaller
├── installer_admin.iss        # Instalador Inno Setup (Program Files, requiere admin)
└── installer_noadmin.iss      # Instalador Inno Setup (AppData, sin admin)
```

## Requisitos

- **Python 3.9 o superior** (solo si corres desde el código fuente; el `.exe` empaquetado no lo necesita).
- **ffmpeg y ffprobe** — ver sección de [binarios de terceros](#binarios-de-terceros-no-incluidos) más abajo.
- **yt-dlp.exe** (opcional, solo si usas el binario standalone en vez de la librería de Python) — ver misma sección.
- **Windows 10/11** (la detección automática de ffmpeg vía WinGet y las rutas del taskbar están pensadas para Windows).

## Binarios de terceros (NO incluidos en este repositorio)

Este repositorio **no incluye** `ffmpeg.exe`, `ffprobe.exe` ni `yt-dlp.exe` porque son binarios pesados de terceros con licencias propias, no código de ClipVault. Debes descargarlos aparte y colocarlos en la **raíz del proyecto** (junto a `main.py`), con esos nombres exactos, antes de correr `build.bat`:

| Binario | De dónde descargarlo |
|---|---|
| `ffmpeg.exe` / `ffprobe.exe` | [gyan.dev/ffmpeg/builds](https://www.gyan.dev/ffmpeg/builds/) (build "full" o "essentials"), o `winget install Gyan.FFmpeg` |
| `yt-dlp.exe` | [github.com/yt-dlp/yt-dlp/releases](https://github.com/yt-dlp/yt-dlp/releases) (archivo `yt-dlp.exe`) |

Si ya tienes ffmpeg instalado vía WinGet en tu PC de desarrollo, ClipVault lo detecta automáticamente sin que hagas nada (`core/config.py` busca en `%LOCALAPPDATA%\Microsoft\WinGet\Packages\Gyan.FFmpeg*`). Para **distribuir** la app a otros usuarios, sin embargo, siempre copia los binarios sueltos junto al `.exe` (ver sección de empaquetado), porque no puedes asumir que el usuario final tiene WinGet configurado igual que tú.

## Instalación (desde código fuente)

```bash
pip install -r requirements.txt
```

Dependencias incluidas: Flask, Flask-SocketIO, yt-dlp (librería), plyer (notificaciones de escritorio), pywebview (ventana nativa), pywin32 (integración con la barra de tareas de Windows).

## Uso en desarrollo

```bash
python main.py
```

Se abre automáticamente una ventana de escritorio con ClipVault (no un navegador).

### Flujo básico

1. En la pestaña "URL", elige el modo: Un solo video, Playlist, o Varios enlaces.
2. Pega el enlace (o varios, uno por línea en modo Varios enlaces) y pulsa Extraer.
3. Los videos válidos aparecen en la lista principal con checkbox; los videos privados, eliminados o con error aparecen en una lista separada de "Videos con problemas".
4. Elige el perfil de calidad activo y la carpeta de destino.
5. Pulsa Descargar seleccionados. La pestaña Descargas muestra la cola en tiempo real con miniaturas, barra de progreso y controles de pausar/cancelar/reordenar.
6. El Historial guarda cada descarga finalizada, con búsqueda, filtro por estado, botón de reintentar y botón para abrir la carpeta de destino.

## Perfiles de calidad

Pestaña Perfiles: perfiles de video predefinidos (Best available, 4K, 1440p, 1080p, 720p, 480p, y variantes Smart TV en MP4/H.264), y perfiles de audio (MP3, M4A/AAC, WAV, FLAC, Opus original). Se pueden editar, resetear a sus valores originales, o crear perfiles personalizados nuevos.

## Funciones avanzadas (pestaña Configuración)

- **SponsorBlock**: salta automáticamente intros, patrocinios y outros.
- **Embeber miniatura y metadatos**: incrusta la carátula y la info del video en el archivo final.
- **Subtítulos/letras**: descarga subtítulos en el idioma elegido, si están disponibles.
- **Cookies** (archivo o navegador): para contenido con restricción de edad o cuentas privadas.
- **Proxy, descargas simultáneas, modo cauteloso** (pausas entre descargas), notificaciones de escritorio.

## Dónde guarda sus datos ClipVault

A diferencia de versiones antiguas, la configuración **no** se guarda junto al ejecutable (evita problemas de permisos si se instala en `Program Files`):

| Archivo | Ubicación | Contenido |
|---|---|---|
| `settings.json` | `%LOCALAPPDATA%\ClipVault\` | Carpeta de destino, ruta de ffmpeg, perfil activo, etc. |
| `profiles.json` | `%LOCALAPPDATA%\ClipVault\` | Perfiles de calidad personalizados |
| `history.json` | `%LOCALAPPDATA%\ClipVault\` | Historial de descargas |
| `logs\clipvault.log` | `%LOCALAPPDATA%\ClipVault\logs\` | Registro rotativo (2 MB x 3 archivos) |

La carpeta de descargas por defecto es la carpeta **Descargas** real del usuario actual (`%USERPROFILE%\Downloads`), a menos que la cambies en Configuración.

## Empaquetado como ejecutable (.exe)

1. Coloca `ffmpeg.exe`, `ffprobe.exe` y `yt-dlp.exe` en la raíz del proyecto (ver sección de binarios de terceros).
2. Corre:
   ```bash
   .\build.bat
   ```
   Esto limpia compilaciones anteriores, corre PyInstaller en modo `--windowed` (sin consola) y copia los tres binarios sueltos junto al `.exe` resultante en `dist\ClipVault\`.
3. Prueba `dist\ClipVault\ClipVault.exe` directamente antes de empaquetar el instalador.

### Generar el instalador (Inno Setup)

Con [Inno Setup](https://jrsoftware.org/isinfo.php) instalado, compila cualquiera de los dos scripts según el caso de uso:

| Script | Instala en | Requiere admin |
|---|---|---|
| `installer_admin.iss` | `C:\Program Files\ClipVault\` | Sí (UAC automático) |
| `installer_noadmin.iss` | `%LOCALAPPDATA%\ClipVault\` | No — ideal para PCs restringidas (empresas, colegios) |

```bash
ISCC.exe installer_admin.iss
ISCC.exe installer_noadmin.iss
```

Los instaladores finales quedan en `installer_output\`.

### Nota sobre SmartScreen/antivirus

La primera vez que se abra el instalador o el `.exe` en una PC nueva, Windows SmartScreen o algún antivirus pueden mostrar una advertencia por no estar firmado digitalmente. Es normal en ejecutables de PyInstaller sin certificado de firma de código pagado — no significa que el programa tenga virus.

## Actualizar yt-dlp

Desde la propia app: pestaña Configuración → botón "Actualizar yt-dlp" (corre `pip install --upgrade yt-dlp` internamente). También manualmente:

```bash
pip install -U yt-dlp
```

## Notas técnicas

- Backend: Flask + Flask-SocketIO (modo threading) para progreso en tiempo real sin bloquear la UI.
- Cola de descargas: prioridad por orden de llegada (FIFO), reordenable arrastrando; concurrencia configurable (1 a 4 descargas simultáneas).
- Reintentos automáticos ante errores transitorios; los errores permanentes (video privado, eliminado, geo-bloqueado) se detectan y no se reintentan innecesariamente.
- Detección de duplicados: no vuelve a descargar un video ya bajado en la misma carpeta de destino.
- Sanitización de nombres de archivo para evitar caracteres inválidos en Windows.
