# ClipVault

Gestor de descargas de video y audio sobre [yt-dlp](https://github.com/yt-dlp/yt-dlp), escrito en **Rust + Slint**: interfaz nativa Material 3, sin Electron ni navegador embebido.

## Instalar

Descarga el instalador desde [Releases](https://github.com/StalFre/ClipVault/releases/latest):

| Archivo | Dónde se instala |
|---|---|
| `ClipVault-Setup-X.Y.Z-Admin.exe` | `C:\Program Files\ClipVault` (todos los usuarios, pide permisos de administrador) |
| `ClipVault-Setup-X.Y.Z-User.exe` | `%LOCALAPPDATA%\Programs\ClipVault` (solo tu usuario, sin administrador) |

El instalador está en español e inglés y, si lo dejas marcado, te guía para activar la **extensión del navegador**.
yt-dlp y ffmpeg se descargan y actualizan solos al abrir la app (en `%LOCALAPPDATA%\ClipVault\bin`).

## Actualizaciones

ClipVault consulta esta página de Releases al iniciar. Si hay una versión nueva, aparece un aviso y en **Ajustes → Acerca de → Instalar** se descarga y se instala sola (respeta la variante Admin o User que tengas).

## Funciones

- Búsqueda en YouTube, enlaces sueltos, listas de reproducción y varios enlaces; los resultados se acumulan por pestaña.
- Cola con pausa, reanudar, cancelar y reordenar arrastrando; historial con reintento.
- Perfiles de video y audio (MP3, M4A, WAV, FLAC, Opus), perfiles propios.
- Recorte por video (con fotogramas de inicio y final), vista previa de 10 s, editor de etiquetas y portada para audio.
- Carpetas por perfil, canal o lista, y plantillas de nombre (`{title} {channel} {id} {profile} {date}`).
- Bandeja del sistema, notificaciones de Windows, progreso en la barra de tareas, detección del portapapeles y arrastrar y soltar enlaces.
- Español / English y modo compacto.
- Extensión de Chrome/Brave/Edge (`ClipVault-extension/`) que ofrece descargar cada video de YouTube cuando la app está abierta.

## Compilar desde el código

Requisitos: Rust (MSVC) y Visual Studio Build Tools con C++.

```
cargo run --release
```

Instaladores (requiere [Inno Setup 6](https://jrsoftware.org/isinfo.php)):

```
./build_installer.ps1
```

Genera `dist/ClipVault-Setup-X.Y.Z-Admin.exe`, `-User.exe` y el zip de la extensión.

## Publicar una versión nueva

1. Sube la versión en `Cargo.toml` (`version = "X.Y.Z"`) y haz commit.
2. `git tag vX.Y.Z && git push origin vX.Y.Z`
3. GitHub Actions compila los instaladores y crea la release; las apps instaladas la detectan solas.

## Datos y registros

`%LOCALAPPDATA%\ClipVault`: `settings.json`, `profiles.json`, `history.json` y `logs\clipvault.log` (rotativo, 1 MB × 5).

La versión anterior en Python/Flask está en el historial de git (commit `1bdbf0c`).
