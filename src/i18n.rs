//! Traduccion de las cadenas generadas en Rust (la UI Slint usa @tr y archivos .po).
//! El texto fuente es espanol; se traduce por fragmentos para cubrir cadenas con numeros.
use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn set(lang: &str) {
    ENGLISH.store(lang == "en", Ordering::Relaxed);
}

pub fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

// Las claves mas largas primero para que no las pisen fragmentos mas cortos.
const EN: &[(&str, &str)] = &[
    ("Hay descargas activas: actualiza cuando terminen", "Downloads are active: update when they finish"),
    ("Pega un enlace válido que empiece con http:// o https://", "Paste a valid link starting with http:// or https://"),
    ("Puerto 5000 ocupado (¿otra copia de ClipVault abierta?). La extensión no funcionará.", "Port 5000 is busy (is another ClipVault open?). The extension will not work."),
    ("Faltan herramientas y no hay conexión para descargarlas", "Tools are missing and there is no connection to download them"),
    ("No se pudieron instalar todas las herramientas (revisa el log)", "Could not install all tools (check the log)"),
    ("Espacio en disco insuficiente (menos de 200 MB libres)", "Not enough disk space (less than 200 MB free)"),
    ("Tiempo de espera agotado consultando el enlace", "Timed out reading the link"),
    ("Sin conexión: se usan las versiones locales", "Offline: using local versions"),
    ("Video privado, eliminado o no disponible", "Private, removed or unavailable video"),
    ("Enlace recibido desde el navegador", "Link received from the browser"),
    ("Lo soltado no contiene un enlace", "What you dropped has no link"),
    ("Tiempo inicial inválido (usa mm:ss)", "Invalid start time (use mm:ss)"),
    ("Tiempo final inválido (usa mm:ss)", "Invalid end time (use mm:ss)"),
    ("El tiempo final debe ser mayor que el inicial", "The end time must be after the start time"),
    ("No se encontraron videos disponibles.", "No available videos found."),
    ("Falta ffplay: pulsa «Buscar actualizaciones» en Ajustes", "ffplay is missing: press “Check for updates” in Settings"),
    ("No se pudo obtener el flujo para la vista previa", "Could not get the stream for the preview"),
    ("No se encontró el archivo", "File not found"),
    ("Etiquetas guardadas", "Tags saved"),
    ("Cargando vista previa…", "Loading preview…"),
    ("Nueva versión disponible", "New version available"),
    ("Estás en la última versión", "You are on the latest version"),
    ("No se pudo comprobar", "Could not check"),
    ("No se pudo actualizar", "Could not update"),
    ("Descargando actualización…", "Downloading update…"),
    ("(Ajustes → Acerca de)", "(Settings → About)"),
    ("Cargando fotograma…", "Loading frame…"),
    ("No se pudo obtener el fotograma", "Could not get the frame"),
    ("Respuesta de yt-dlp inválida", "Invalid yt-dlp response"),
    ("No se pudo acceder al video", "Could not access the video"),
    ("Video privado o no disponible", "Private or unavailable video"),
    ("Elemento no disponible", "Unavailable item"),
    ("Descarga completa", "Download complete"),
    ("Incrustando metadatos…", "Embedding metadata…"),
    ("Quitando patrocinios…", "Removing sponsors…"),
    ("Uniendo audio y video…", "Merging audio and video…"),
    ("Comprobando herramientas…", "Checking tools…"),
    ("Herramientas actualizadas", "Tools updated"),
    ("Herramientas al día", "Tools up to date"),
    ("Extrayendo ffmpeg…", "Extracting ffmpeg…"),
    ("Descargando", "Downloading"),
    ("Reintentando", "Retrying"),
    ("Convirtiendo…", "Converting…"),
    ("Preparando…", "Preparing…"),
    ("Cancelando…", "Cancelling…"),
    ("En pausa", "Paused"),
    ("En cola", "Queued"),
    ("elemento(s)", "item(s)"),
    ("añadido(s) a la cola", "added to the queue"),
    ("Añadido de nuevo a la cola", "Added to the queue again"),
    ("Escribe algo para buscar", "Type something to search"),
    ("Pega al menos un enlace", "Paste at least one link"),
    ("Cola vaciada", "Queue cleared"),
    ("La carpeta ya no existe", "The folder no longer exists"),
    ("La carpeta aún no existe", "The folder does not exist yet"),
    ("Historial borrado", "History cleared"),
    ("Perfil creado", "Profile created"),
    ("no disponible(s), p. ej.", "unavailable, e.g."),
    ("No es un enlace válido", "Not a valid link"),
    ("No se generó el archivo", "The file was not generated"),
    ("Revisa que ffmpeg funcione.", "Check that ffmpeg works."),
    ("Sin título", "Untitled"),
    ("Falló:", "Failed:"),
    ("Mejor calidad", "Best quality"),
    ("Hasta", "Up to"),
    ("formatos nativos", "native formats"),
    ("Solo audio · MP3 192 kbps · máxima compatibilidad", "Audio only · MP3 192 kbps · maximum compatibility"),
    ("Solo audio · M4A/AAC 192 kbps · ideal para iPhone e iTunes", "Audio only · M4A/AAC 192 kbps · ideal for iPhone and iTunes"),
    ("Solo audio · WAV sin pérdida · archivo pesado", "Audio only · lossless WAV · large file"),
    ("Solo audio · FLAC sin pérdida · comprimido", "Audio only · lossless FLAC · compressed"),
    ("Solo audio · Opus original · sin recodificar", "Audio only · original Opus · no re-encoding"),
    ("Solo audio", "Audio only"),
    ("Descargando", "Downloading"),
    ("Vista previa", "Preview"),
    ("Mostrar ClipVault", "Show ClipVault"),
    ("Salir", "Quit"),
    ("Enlace copiado", "Copied link"),
    ("Enlace soltado", "Dropped link"),
    ("Enlace recibido", "Link received"),
    ("Sin canal", "No channel"),
    ("Sin portada nueva", "No new cover"),
];

pub fn t(s: &str) -> String {
    if !is_english() {
        return s.to_string();
    }
    let mut out = s.to_string();
    for (es, en) in EN {
        if out.contains(es) {
            out = out.replace(es, en);
        }
    }
    out
}
