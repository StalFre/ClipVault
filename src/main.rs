#![cfg_attr(windows, windows_subsystem = "windows")]

mod args;
mod config;
mod core;
mod extractor;
mod history;
mod i18n;
mod logger;
mod media;
mod model;
mod naming;
mod notify;
mod paths;
mod server;
mod sys;
mod thumbs;
mod tools;
mod updater;
mod util;

use crate::core::{Core, Event, ExtractMode, Status};
use crate::i18n::t;
use crate::model::{Extracted, Profile, Video};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

slint::include_modules!();

const RATES: [u32; 5] = [0, 500, 1024, 2048, 5120];
const CAUTIOUS: [&str; 3] = ["off", "balanced", "careful"];
const COOKIES: [&str; 3] = ["off", "file", "browser"];
const BROWSERS: [&str; 4] = ["chrome", "firefox", "edge", "brave"];
const SUB_LANGS: [&str; 11] = ["es", "en", "es,en", "en,es", "pt", "fr", "de", "it", "ja", "ko", "zh"];
const SEARCH_COUNTS: [u32; 4] = [10, 15, 25, 40];
const HEIGHTS: [Option<u32>; 7] = [None, Some(2160), Some(1440), Some(1080), Some(720), Some(480), Some(360)];
const CODECS: [&str; 5] = ["mp3", "m4a", "wav", "flac", "opus"];
const LANGS: [&str; 2] = ["es", "en"];
const MAX_HISTORY_ROWS: usize = 150;
const TOAST_MS: u64 = 3200;

#[derive(Clone, Default)]
struct Entry {
    video: Video,
    selected: bool,
    from: String,
    to: String,
}

/// 0 = busqueda, 1 = un video, 2 = lista, 3 = varios enlaces
#[derive(Default)]
struct List {
    items: Vec<Entry>,
    playlist_title: String,
    busy: bool,
    error: String,
    warning: String,
}

const LISTS: usize = 4;

struct App {
    core: Arc<Core>,
    ui: slint::Weak<MainWindow>,
    /// None = la descarga fallo (se muestra el placeholder, sin reintentos)
    cache: HashMap<String, Option<Image>>,
    lists: [List; LISTS],
    list_models: [Rc<VecModel<VideoRow>>; LISTS],
    trim_target: Option<(usize, usize)>,
    queue_model: Rc<VecModel<QueueRow>>,
    history_model: Rc<VecModel<HistoryRow>>,
    profile_model: Rc<VecModel<ProfileRow>>,
    profiles: Vec<Profile>,
    toasts: VecDeque<String>,
    toast_until: Instant,
    // sistema
    clipboard: Option<arboard::Clipboard>,
    last_clip: String,
    last_clip_check: Instant,
    tray: Option<sys::Tray>,
    #[cfg(windows)]
    taskbar: sys::win::Taskbar,
    last_progress: Option<i32>,
    drop_hwnd: isize,
    // editor de etiquetas
    tag_target: Option<PathBuf>,
    tag_cover: Option<PathBuf>,
    app_update: Option<updater::UpdateInfo>,
}

type Shared = Rc<RefCell<App>>;

fn thumb_for(cache: &mut HashMap<String, Option<Image>>, core: &Core, url: Option<&str>) -> (Image, bool) {
    let Some(u) = url.filter(|u| !u.is_empty()) else { return (Image::default(), false) };
    match cache.get(u) {
        Some(Some(img)) => (img.clone(), true),
        Some(None) => (Image::default(), false),
        None => {
            core.thumbs.request(u);
            (Image::default(), false)
        }
    }
}

fn pretty_date(stamp: &str) -> String {
    stamp.replace('T', " ").chars().take(16).collect()
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn hwnd_of(ui: &MainWindow) -> Option<isize> {
    let h = ui.window().window_handle();
    match h.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(w) => Some(w.hwnd.get()),
        _ => None,
    }
}

fn show_window(ui: &MainWindow) {
    let _ = ui.show();
    ui.window().set_minimized(false);
}

impl App {
    fn ui(&self) -> Option<MainWindow> {
        self.ui.upgrade()
    }

    fn toast(&mut self, msg: &str) {
        self.toasts.push_back(msg.to_string());
    }

    // ---------------------------------------------------------------- listas de resultados

    fn rebuild_list(&mut self, idx: usize) {
        let items = self.lists[idx].items.clone();
        let rows: Vec<VideoRow> = items
            .iter()
            .map(|e| {
                let v = &e.video;
                let (thumb, has) = thumb_for(&mut self.cache, &self.core, v.thumbnail.as_deref());
                let mut meta = Vec::new();
                if let Some(d) = v.duration {
                    meta.push(util::fmt_duration(d));
                }
                if !v.channel.is_empty() {
                    meta.push(v.channel.clone());
                }
                if !v.extractor.is_empty() {
                    meta.push(title_case(&v.extractor));
                }
                if !e.from.is_empty() || !e.to.is_empty() {
                    let to = if e.to.is_empty() { "fin".to_string() } else { e.to.clone() };
                    let from = if e.from.is_empty() { "0:00".to_string() } else { e.from.clone() };
                    meta.push(format!("✂ {from}–{to}"));
                }
                VideoRow { title: v.title.as_str().into(), meta: meta.join(" · ").into(), thumb, has_thumb: has, selected: e.selected }
            })
            .collect();
        sync_model(&self.list_models[idx], rows);
        if let Some(ui) = self.ui() {
            let n = items.iter().filter(|e| e.selected).count() as i32;
            let b = ui.global::<Backend>();
            if idx == 0 {
                b.set_search_selected(n);
            } else if idx == (b.get_url_mode().clamp(0, 2) as usize) + 1 {
                b.set_url_selected(n);
            }
        }
    }

    fn rebuild_queue(&mut self) {
        struct Snap {
            id: u64,
            title: String,
            thumbnail: Option<String>,
            status: Status,
            pct: f32,
            speed: String,
            eta: String,
            stage: String,
        }
        let snaps: Vec<Snap> = self
            .core
            .items
            .lock()
            .map(|items| {
                items
                    .iter()
                    .map(|i| Snap {
                        id: i.id,
                        title: i.video.title.clone(),
                        thumbnail: i.video.thumbnail.clone(),
                        status: i.status,
                        pct: i.pct,
                        speed: i.speed.clone(),
                        eta: i.eta.clone(),
                        stage: i.stage.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let n = snaps.len();
        let rows: Vec<QueueRow> = snaps
            .into_iter()
            .enumerate()
            .map(|(pos, s)| {
                let (thumb, has) = thumb_for(&mut self.cache, &self.core, s.thumbnail.as_deref());
                let downloading = s.status == Status::Downloading;
                let transferring = downloading && s.stage == "Descargando";
                let detail = if transferring {
                    let mut d = vec![format!("{:.0}%", s.pct)];
                    if !s.speed.is_empty() {
                        d.push(s.speed.clone());
                    }
                    if !s.eta.is_empty() {
                        d.push(format!("ETA {}", s.eta));
                    }
                    d.join(" · ")
                } else {
                    String::new()
                };
                let status = match s.status {
                    Status::Queued => "En cola".to_string(),
                    Status::Paused => "En pausa".to_string(),
                    Status::Cancelling => "Cancelando…".to_string(),
                    Status::Downloading => s.stage.clone(),
                };
                QueueRow {
                    id: s.id as i32,
                    title: s.title.as_str().into(),
                    status: t(&status).as_str().into(),
                    detail: detail.as_str().into(),
                    pct: s.pct,
                    thumb,
                    has_thumb: has,
                    paused: s.status == Status::Paused,
                    busy: (downloading && !transferring) || s.status == Status::Cancelling,
                    can_pause: downloading || s.status == Status::Queued,
                    can_up: pos > 0,
                    can_down: pos + 1 < n,
                }
            })
            .collect();
        sync_model(&self.queue_model, rows);
    }

    fn rebuild_history(&mut self) {
        let Some(ui) = self.ui() else { return };
        let b = ui.global::<Backend>();
        let q = b.get_hist_query().to_lowercase();
        let filter = match b.get_hist_filter() {
            1 => "done",
            2 => "failed",
            _ => "",
        };
        let all = self.core.history.lock().map(|h| h.clone()).unwrap_or_default();
        b.set_history_total(all.len() as i32);
        let rows: Vec<HistoryRow> = all
            .into_iter()
            .filter(|r| (filter.is_empty() || r.status == filter) && (q.is_empty() || r.title.to_lowercase().contains(&q)))
            .take(MAX_HISTORY_ROWS)
            .map(|r| {
                let (thumb, has) = thumb_for(&mut self.cache, &self.core, r.video.thumbnail.as_deref());
                let kind = match r.status.as_str() {
                    "done" => 0,
                    "skipped" => 2,
                    _ => 1,
                };
                HistoryRow {
                    id: r.item_id.as_str().into(),
                    title: r.title.as_str().into(),
                    meta: format!("{} · {}", t(&r.profile.name), pretty_date(&r.finished_at)).into(),
                    error: t(&r.error.clone().unwrap_or_default()).as_str().into(),
                    can_edit: !r.file.is_empty() && std::path::Path::new(&r.file).is_file(),
                    kind,
                    thumb,
                    has_thumb: has,
                }
            })
            .collect();
        sync_model(&self.history_model, rows);
    }

    fn rebuild_profiles(&mut self) {
        self.profiles = self.core.profiles();
        let active = self.core.settings().active_profile_id;
        let idx = self.profiles.iter().position(|p| p.id == active).unwrap_or(0);
        let rows: Vec<ProfileRow> = self
            .profiles
            .iter()
            .map(|p| ProfileRow {
                id: p.id.as_str().into(),
                name: t(&p.name).as_str().into(),
                desc: t(&p.describe()).as_str().into(),
                audio: p.is_audio(),
                builtin: p.builtin,
                active: p.id == active,
            })
            .collect();
        self.profile_model.set_vec(rows);
        if let Some(ui) = self.ui() {
            let b = ui.global::<Backend>();
            let names: Vec<SharedString> = self.profiles.iter().map(|p| t(&p.name).as_str().into()).collect();
            b.set_profile_names(ModelRc::new(VecModel::from(names)));
            b.set_profile_index(idx as i32);
            b.set_profile_desc(self.profiles.get(idx).map(|p| t(&p.describe())).unwrap_or_default().as_str().into());
        }
    }

    fn current_profile(&self) -> Option<Profile> {
        let idx = self.ui()?.global::<Backend>().get_profile_index();
        self.profiles.get(idx.max(0) as usize).cloned()
    }

    fn rebuild_all(&mut self) {
        self.rebuild_list(0);
        self.rebuild_list(1);
        self.rebuild_queue();
        self.rebuild_history();
    }

    // ---------------------------------------------------------------- eventos

    fn on_extracted(&mut self, list: usize, result: Result<Extracted, String>) {
        let Some(ui) = self.ui() else { return };
        let b = ui.global::<Backend>();
        let list = list.min(LISTS - 1);
        {
            let l = &mut self.lists[list];
            l.busy = false;
            l.error.clear();
            l.warning.clear();
            match result {
                Ok(ex) => {
                    if ex.items.is_empty() {
                        l.error = t("No se encontraron videos disponibles.");
                    }
                    if !ex.skipped.is_empty() {
                        let first = &ex.skipped[0];
                        l.warning = t(&format!("{} no disponible(s), p. ej. «{}»: {}", ex.skipped.len(), first.title, first.reason));
                    }
                    if list == 0 {
                        l.items.clear(); // la busqueda se reemplaza
                    }
                    if let Some(pt) = ex.playlist_title.clone() {
                        l.playlist_title = pt;
                    }
                    for mut v in ex.items {
                        // los enlaces de varias fuentes se acumulan sin duplicar
                        if l.items.iter().any(|e| e.video.url == v.url || (!v.id.is_empty() && e.video.id == v.id)) {
                            continue;
                        }
                        if let Some(pt) = &ex.playlist_title {
                            v.playlist = pt.clone();
                        }
                        l.items.push(Entry { video: v, selected: list != 0, from: String::new(), to: String::new() });
                    }
                }
                Err(e) => l.error = t(&e),
            }
        }
        if list == 0 {
            b.set_search_busy(false);
            b.set_search_error(self.lists[0].error.as_str().into());
        } else {
            self.apply_url_view(&ui);
        }
        self.rebuild_list(list);
    }

    /// Muestra en la pagina Enlace el estado de la lista que corresponde al modo elegido.
    fn apply_url_view(&mut self, ui: &MainWindow) {
        let b = ui.global::<Backend>();
        let mode = b.get_url_mode().clamp(0, 2) as usize;
        let idx = mode + 1;
        b.set_url_rows(ModelRc::from(self.list_models[idx].clone()));
        let l = &self.lists[idx];
        b.set_url_busy(l.busy);
        b.set_url_error(l.error.as_str().into());
        b.set_skipped_summary(l.warning.as_str().into());
        b.set_url_title(if mode == 1 { l.playlist_title.as_str().into() } else { "".into() });
        b.set_url_selected(l.items.iter().filter(|e| e.selected).count() as i32);
    }

    fn offer_link(&mut self, ui: &MainWindow, url: &str, title: &str) {
        let b = ui.global::<Backend>();
        b.set_ext_title(t(title).as_str().into());
        b.set_ext_url(url.into());
        b.set_ext_dialog(true);
    }

    /// Carga el enlace en la lista que corresponde (modo 0 = video, 1 = lista, 2 = varios) y muestra la pagina.
    fn start_extract(&mut self, ui: &MainWindow, mode: ExtractMode, input: String, mode_idx: i32) {
        let b = ui.global::<Backend>();
        b.set_page(1);
        b.set_url_mode(mode_idx);
        let idx = mode_idx.clamp(0, 2) as usize + 1;
        self.lists[idx].busy = true;
        self.lists[idx].error.clear();
        self.apply_url_view(ui);
        self.core.extract_async(idx, mode, input);
    }

    /// Enlace copiado/soltado: video suelto -> pestana "Un solo video"; lista pura -> pestana "Lista".
    fn auto_extract(&mut self, ui: &MainWindow, url: &str) {
        let has_list = url.contains("list=") || url.contains("/playlist");
        let has_video = url.contains("v=") || url.contains("youtu.be/") || url.contains("/shorts/");
        if has_list && !has_video {
            self.start_extract(ui, ExtractMode::Playlist, url.to_string(), 1);
        } else {
            self.start_extract(ui, ExtractMode::Single, url.to_string(), 0);
        }
    }

    fn check_clipboard(&mut self, ui: &MainWindow) {
        if self.last_clip_check.elapsed() < Duration::from_millis(1200) {
            return;
        }
        self.last_clip_check = Instant::now();
        let on = self.core.settings().clipboard_watch;
        let Some(cb) = self.clipboard.as_mut() else { return };
        let Ok(text) = cb.get_text() else { return };
        let text = text.trim().to_string();
        if text == self.last_clip {
            return;
        }
        self.last_clip = text.clone();
        let b = ui.global::<Backend>();
        if !on || !ui.window().is_visible() || b.get_ext_dialog() || text.len() > 600 {
            return;
        }
        let urls = sys::urls_in(&text);
        if urls.len() == 1 && sys::looks_like_media_url(&urls[0]) {
            let u = urls[0].clone();
            self.toast("Enlace copiado: cargando…");
            self.auto_extract(ui, &u);
        }
    }

    fn update_taskbar(&mut self, ui: &MainWindow) {
        let prog: Option<i32> = {
            let items = self.core.items.lock().map(|i| i.iter().map(|x| x.pct).collect::<Vec<f32>>()).unwrap_or_default();
            if items.is_empty() { None } else { Some((items.iter().sum::<f32>() / items.len() as f32 * 10.0) as i32) }
        };
        if prog == self.last_progress {
            return;
        }
        self.last_progress = prog;
        #[cfg(windows)]
        if let Some(h) = hwnd_of(ui) {
            self.taskbar.set(h, prog.map(|p| p as f32 / 1000.0));
        }
        #[cfg(not(windows))]
        let _ = ui;
    }

    fn tick(&mut self) {
        let Some(ui) = self.ui() else { return };

        for ev in self.core.take_events() {
            match ev {
                Event::Toast(m) => self.toast(&m),
                Event::ExtLink { url, mode } => {
                    match mode {
                        Some(m) => {
                            let em = if m == 1 { ExtractMode::Playlist } else { ExtractMode::Single };
                            self.start_extract(&ui, em, url, m as i32);
                            self.toast("Enlace recibido desde el navegador");
                        }
                        None => self.offer_link(&ui, &url, "Enlace recibido"),
                    }
                    show_window(&ui);
                }
                Event::Dropped(urls) => {
                    if urls.len() == 1 {
                        self.auto_extract(&ui, &urls[0]);
                    } else {
                        ui.global::<Backend>().set_bulk_text(urls.join("\n").as_str().into());
                        self.start_extract(&ui, ExtractMode::Bulk, urls.join("\n"), 2);
                    }
                }
                Event::TagDone(r) => {
                    ui.global::<Backend>().set_tag_busy(false);
                    match r {
                        Ok(()) => {
                            ui.global::<Backend>().set_tag_dialog(false);
                            self.toast("Etiquetas guardadas");
                        }
                        Err(e) => self.toast(&e),
                    }
                }
                Event::TrimFrame(r) => match r {
                    Ok((px, w, h)) => {
                        let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&px, w, h);
                        let b = ui.global::<Backend>();
                        b.set_trim_thumb(Image::from_rgba8(buf));
                        b.set_trim_has_thumb(true);
                    }
                    Err(e) => self.toast(&e),
                },
                Event::Quit => {
                    let _ = slint::quit_event_loop();
                }
                Event::AppUpdate { result, manual } => {
                    let b = ui.global::<Backend>();
                    b.set_app_update_busy(false);
                    match result {
                        Ok(Some(info)) => {
                            let msg = format!("Nueva versión disponible: {}", info.version);
                            b.set_app_update_status(t(&msg).as_str().into());
                            b.set_app_update_available(true);
                            self.toast(&format!("{msg} (Ajustes → Acerca de)"));
                            self.app_update = Some(info);
                        }
                        Ok(None) => {
                            b.set_app_update_status(t("Estás en la última versión").as_str().into());
                            b.set_app_update_available(false);
                            if manual {
                                self.toast("Estás en la última versión");
                            }
                        }
                        Err(e) => {
                            b.set_app_update_status(t("No se pudo comprobar").as_str().into());
                            if manual {
                                self.toast(&format!("No se pudo comprobar: {e}"));
                            }
                        }
                    }
                }
                Event::Extracted { list, result } => self.on_extracted(list, result),
            }
        }

        let decoded = self.core.thumbs.drain();
        if !decoded.is_empty() {
            for d in decoded {
                let img = d.rgba.map(|px| {
                    let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&px, thumbs::THUMB_W, thumbs::THUMB_H);
                    Image::from_rgba8(buf)
                });
                self.cache.insert(d.url, img);
            }
            self.rebuild_all();
        }
        if self.core.queue_dirty.swap(false, Ordering::Relaxed) {
            self.rebuild_queue();
        }
        if self.core.history_dirty.swap(false, Ordering::Relaxed) {
            self.rebuild_history();
        }

        let tl = self.core.tools.lock().map(|t| t.clone()).unwrap_or_default();
        let b = ui.global::<Backend>();
        b.set_ytdlp_version(tl.ytdlp.as_str().into());
        b.set_ffmpeg_version(tl.ffmpeg.as_str().into());
        b.set_tools_status(t(&tl.status).as_str().into());
        b.set_tools_busy(tl.busy);
        b.set_tools_progress(tl.progress);

        // bandeja
        let actions = self.tray.as_ref().map(|t| t.poll()).unwrap_or_default();
        for a in actions {
            match a {
                sys::TrayAction::Show => show_window(&ui),
                sys::TrayAction::Quit => {
                    let _ = slint::quit_event_loop();
                }
            }
        }

        self.check_clipboard(&ui);
        self.update_taskbar(&ui);

        // (re)registra el destino de arrastre cuando cambia el HWND
        #[cfg(windows)]
        if ui.window().is_visible() {
            if let Some(h) = hwnd_of(&ui) {
                if h != self.drop_hwnd {
                    self.drop_hwnd = h;
                    sys::win::register_drop(h, self.core.clone());
                }
            }
        }

        let now = Instant::now();
        if now >= self.toast_until {
            if let Some(m) = self.toasts.pop_front() {
                b.set_toast_text(t(&m).as_str().into());
                b.set_toast_visible(true);
                self.toast_until = now + Duration::from_millis(TOAST_MS);
            } else {
                b.set_toast_visible(false);
            }
        }
    }
}

/// Actualiza solo las filas que cambiaron para no perder hover/estado en la UI.
fn sync_model<T: Clone + PartialEq + 'static>(model: &Rc<VecModel<T>>, rows: Vec<T>) {
    if model.row_count() != rows.len() {
        model.set_vec(rows);
        return;
    }
    for (i, r) in rows.into_iter().enumerate() {
        if model.row_data(i).as_ref() != Some(&r) {
            model.set_row_data(i, r);
        }
    }
}

fn open_in_explorer(path: &std::path::Path) {
    let _ = std::process::Command::new("explorer").arg(path).spawn();
}

fn apply_language(code: &str) {
    i18n::set(code);
    if slint::select_bundled_translation(code).is_err() {
        log::warn!("traducción no encontrada: {code}");
    }
}

fn load_settings_into_ui(ui: &MainWindow, core: &Core) {
    let s = core.settings();
    let b = ui.global::<Backend>();
    b.set_dark(s.theme != "light");
    b.set_dest_dir(s.download_dir.as_str().into());
    b.set_cookies_mode(COOKIES.iter().position(|c| *c == s.cookies_mode).unwrap_or(0) as i32);
    b.set_cookies_file(s.cookies_file.as_str().into());
    b.set_cookies_browser_idx(BROWSERS.iter().position(|c| *c == s.cookies_browser).unwrap_or(0) as i32);
    b.set_proxy_url(s.proxy_url.as_str().into());
    b.set_playlist_limit(s.playlist_scan_limit as i32);
    b.set_rate_idx(RATES.iter().position(|r| *r == s.rate_limit_kbps).unwrap_or(0) as i32);
    b.set_conc_idx(s.concurrency.clamp(1, 4) as i32 - 1);
    b.set_cautious_idx(CAUTIOUS.iter().position(|c| *c == s.cautious_mode).unwrap_or(1) as i32);
    b.set_sponsorblock(s.sponsorblock_remove);
    b.set_normalize(s.normalize_volume);
    b.set_embed(s.embed_thumbnail_metadata);
    b.set_subs(s.download_subtitles);
    b.set_subs_lang_idx(SUB_LANGS.iter().position(|c| *c == s.subtitle_langs).unwrap_or(2) as i32);
    b.set_auto_update(s.auto_update_tools);
    b.set_clipboard_watch(s.clipboard_watch);
    b.set_close_to_tray(s.close_to_tray);
    b.set_notifications(s.notifications);
    b.set_folder_idx(s.folder_mode.min(3) as i32);
    b.set_name_template(s.name_template.as_str().into());
    b.set_lang_idx(LANGS.iter().position(|c| *c == s.language).unwrap_or(0) as i32);
    b.set_compact_pref(s.compact_mode);
}

fn pick<T: Copy>(arr: &[T], idx: i32, default: T) -> T {
    arr.get(idx.max(0) as usize).copied().unwrap_or(default)
}

fn wire(ui: &MainWindow, app: &Shared) {
    let b = ui.global::<Backend>();
    let core = app.borrow().core.clone();

    // ---- busqueda / extraccion
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_search(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            let b = ui.global::<Backend>();
            let q = b.get_search_query().to_string();
            if q.trim().is_empty() {
                app.borrow_mut().toast("Escribe algo para buscar");
                return;
            }
            let n = pick(&SEARCH_COUNTS, b.get_search_count_idx(), 15);
            b.set_search_busy(true);
            b.set_search_error("".into());
            app.borrow().core.extract_async(0, ExtractMode::Search(n), q);
        });
    }
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_extract_url(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            let b = ui.global::<Backend>();
            let mode_idx = b.get_url_mode().clamp(0, 2);
            let (mode, input) = match mode_idx {
                2 => (ExtractMode::Bulk, b.get_bulk_text().to_string()),
                1 => (ExtractMode::Playlist, b.get_url_text().to_string()),
                _ => (ExtractMode::Single, b.get_url_text().to_string()),
            };
            if input.trim().is_empty() {
                app.borrow_mut().toast("Pega al menos un enlace");
                return;
            }
            app.borrow_mut().start_extract(&ui, mode, input, mode_idx);
            // el texto se limpia para poder pegar el siguiente enlace y acumular
            if mode_idx != 2 {
                b.set_url_text("".into());
            }
        });
    }
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_url_mode_changed(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            app.borrow_mut().apply_url_view(&ui);
        });
    }
    {
        let app = app.clone();
        b.on_toggle_select(move |list, idx| {
            let mut a = app.borrow_mut();
            let l = list.clamp(0, 3) as usize;
            if let Some(e) = a.lists[l].items.get_mut(idx as usize) {
                e.selected = !e.selected;
            }
            a.rebuild_list(l);
        });
    }
    {
        let app = app.clone();
        b.on_select_all(move |list, v| {
            let mut a = app.borrow_mut();
            let l = list.clamp(0, 3) as usize;
            a.lists[l].items.iter_mut().for_each(|e| e.selected = v);
            a.rebuild_list(l);
        });
    }
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_clear_list(move |list| {
            let mut a = app.borrow_mut();
            let l = list.clamp(0, 3) as usize;
            a.lists[l] = List::default();
            if let Some(ui) = ui_w.upgrade() {
                if l == 0 {
                    ui.global::<Backend>().set_search_error("".into());
                } else {
                    a.apply_url_view(&ui);
                }
            }
            a.rebuild_list(l);
        });
    }
    {
        let app = app.clone();
        b.on_remove_unselected(move |list| {
            let mut a = app.borrow_mut();
            let l = list.clamp(0, 3) as usize;
            a.lists[l].items.retain(|e| e.selected);
            a.rebuild_list(l);
        });
    }
    {
        let app = app.clone();
        b.on_download(move |list| {
            let mut a = app.borrow_mut();
            let l = list.clamp(0, 3) as usize;
            let Some(profile) = a.current_profile() else { return };
            let mut videos = Vec::new();
            let mut ranges = Vec::new();
            for e in a.lists[l].items.iter().filter(|e| e.selected) {
                match naming::section_arg(&e.from, &e.to) {
                    Ok(r) => ranges.push(r),
                    Err(m) => {
                        let msg = format!("{}: {m}", e.video.title);
                        drop(videos);
                        a.toast(&msg);
                        return;
                    }
                }
                videos.push(e.video.clone());
            }
            if videos.is_empty() {
                return;
            }
            let queued = a.core.enqueue(&videos, &profile, None, &ranges);
            a.lists[l].items.iter_mut().for_each(|e| e.selected = false);
            a.toast(&format!("{queued} añadido(s) a la cola"));
            a.rebuild_list(l);
        });
    }

    // ---- vista previa y recorte por video
    {
        let app = app.clone();
        b.on_preview(move |list, idx, with_video| {
            let a = app.borrow();
            let Some(v) = a.lists[list.clamp(0, 3) as usize].items.get(idx as usize).map(|e| e.video.clone()) else { return };
            let core = a.core.clone();
            core.toast("Cargando vista previa…");
            std::thread::spawn(move || {
                if let Err(e) = media::preview(&v, &core.settings(), with_video, None) {
                    core.toast(&e);
                }
            });
        });
    }
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_trim_open(move |list, idx| {
            let Some(ui) = ui_w.upgrade() else { return };
            let mut a = app.borrow_mut();
            let l = list.clamp(0, 3) as usize;
            let Some(e) = a.lists[l].items.get(idx as usize).cloned() else { return };
            let core = a.core.clone();
            let (thumb, has) = thumb_for(&mut a.cache, &core, e.video.thumbnail.as_deref());
            let b = ui.global::<Backend>();
            b.set_trim_title(e.video.title.as_str().into());
            b.set_trim_thumb(thumb);
            b.set_trim_has_thumb(has);
            b.set_trim_duration(e.video.duration.map(util::fmt_duration).unwrap_or_default().as_str().into());
            b.set_trim_from(e.from.as_str().into());
            b.set_trim_to(e.to.as_str().into());
            b.set_trim_dialog(true);
            a.trim_target = Some((l, idx as usize));
        });
    }
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_trim_apply(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            let mut a = app.borrow_mut();
            let b = ui.global::<Backend>();
            let (from, to) = (b.get_trim_from().trim().to_string(), b.get_trim_to().trim().to_string());
            if let Err(e) = naming::section_arg(&from, &to) {
                a.toast(&e);
                return;
            }
            if let Some((l, i)) = a.trim_target {
                if let Some(e) = a.lists[l].items.get_mut(i) {
                    e.from = from;
                    e.to = to;
                }
                a.rebuild_list(l);
            }
            b.set_trim_dialog(false);
        });
    }
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_trim_clear(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            let mut a = app.borrow_mut();
            if let Some((l, i)) = a.trim_target {
                if let Some(e) = a.lists[l].items.get_mut(i) {
                    e.from.clear();
                    e.to.clear();
                }
                a.rebuild_list(l);
            }
            ui.global::<Backend>().set_trim_dialog(false);
        });
    }
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_trim_preview(move |from_start| {
            let Some(ui) = ui_w.upgrade() else { return };
            let a = app.borrow();
            let Some((l, i)) = a.trim_target else { return };
            let Some(v) = a.lists[l].items.get(i).map(|e| e.video.clone()) else { return };
            let b = ui.global::<Backend>();
            let start = if from_start {
                naming::parse_time(b.get_trim_from().as_str()).unwrap_or(0)
            } else {
                naming::parse_time(b.get_trim_to().as_str()).map(|t| t.saturating_sub(10)).unwrap_or_else(|| v.duration.map(|d| (d as u32).saturating_sub(10)).unwrap_or(0))
            };
            let core = a.core.clone();
            core.toast("Cargando vista previa…");
            std::thread::spawn(move || {
                if let Err(e) = media::preview(&v, &core.settings(), true, Some(start)) {
                    core.toast(&e);
                }
            });
        });
    }

    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_trim_frame(move |from_start| {
            let Some(ui) = ui_w.upgrade() else { return };
            let a = app.borrow();
            let Some((l, i)) = a.trim_target else { return };
            let Some(v) = a.lists[l].items.get(i).map(|e| e.video.clone()) else { return };
            let b = ui.global::<Backend>();
            let secs = if from_start {
                naming::parse_time(b.get_trim_from().as_str()).unwrap_or(0)
            } else {
                naming::parse_time(b.get_trim_to().as_str()).unwrap_or_else(|| v.duration.map(|d| (d as u32).saturating_sub(2)).unwrap_or(0))
            };
            let core = a.core.clone();
            core.toast("Cargando fotograma…");
            std::thread::spawn(move || {
                let r = media::frame_at(&v, &core.settings(), secs);
                core.push_event(Event::TrimFrame(r));
            });
        });
    }

    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_check_app_update(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            ui.global::<Backend>().set_app_update_busy(true);
            app.borrow().core.check_app_update(true);
        });
    }
    {
        let app = app.clone();
        b.on_install_app_update(move || {
            let a = app.borrow();
            if let Some(info) = a.app_update.clone() {
                a.core.install_app_update(info);
            }
        });
    }

    // ---- cola
    {
        let core = core.clone();
        b.on_pause(move |id| core.pause(id as u64));
    }
    {
        let core = core.clone();
        b.on_resume(move |id| core.resume(id as u64));
    }
    {
        let core = core.clone();
        b.on_cancel(move |id| core.cancel(id as u64));
    }
    {
        let core = core.clone();
        b.on_drag_move(move |id, steps| core.move_item(id as u64, steps.round() as i32));
    }
    {
        let core = core.clone();
        b.on_open_queue_folder(move |id| match core.item_dest(id as u64) {
            Some(d) if std::path::Path::new(&d).is_dir() => open_in_explorer(std::path::Path::new(&d)),
            _ => core.toast("La carpeta aún no existe"),
        });
    }
    {
        let core = core.clone();
        b.on_clear_queue(move || {
            let n = core.clear_queue();
            core.toast(&format!("Cola vaciada ({n})"));
        });
    }

    // ---- historial
    {
        let core = core.clone();
        b.on_retry(move |id| {
            core.retry(id.as_str());
            core.toast("Añadido de nuevo a la cola");
        });
    }
    {
        let core = core.clone();
        b.on_open_folder(move |id| {
            let dir = core
                .history
                .lock()
                .ok()
                .and_then(|h| h.iter().find(|r| r.item_id == id.as_str()).map(|r| r.dest_dir.clone()));
            match dir {
                Some(d) if std::path::Path::new(&d).is_dir() => open_in_explorer(std::path::Path::new(&d)),
                _ => core.toast("La carpeta ya no existe"),
            }
        });
    }
    {
        let core = core.clone();
        b.on_clear_history(move || {
            core.clear_history();
            core.toast("Historial borrado");
        });
    }
    {
        let core = core.clone();
        b.on_history_changed(move || core.history_dirty.store(true, Ordering::Relaxed));
    }

    // ---- editor de etiquetas
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_edit_tags(move |id| {
            let Some(ui) = ui_w.upgrade() else { return };
            let mut a = app.borrow_mut();
            let rec = a.core.history.lock().ok().and_then(|h| h.iter().find(|r| r.item_id == id.as_str()).cloned());
            let Some(rec) = rec else { return };
            let path = PathBuf::from(&rec.file);
            if !path.is_file() {
                a.toast("No se encontró el archivo");
                return;
            }
            let mut tags = media::read_tags(&path);
            if tags.title.is_empty() && tags.artist.is_empty() {
                let (artist, title) = media::guess_artist_title(&rec.title);
                tags.artist = if artist.is_empty() { rec.video.channel.clone() } else { artist };
                tags.title = title;
            }
            let b = ui.global::<Backend>();
            b.set_tag_title(tags.title.as_str().into());
            b.set_tag_artist(tags.artist.as_str().into());
            b.set_tag_album(tags.album.as_str().into());
            b.set_tag_busy(false);
            b.set_tag_dialog(true);
            a.tag_target = Some(path);
            a.tag_cover = None;
        });
    }
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_tag_pick_cover(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            if let Some(f) = rfd::FileDialog::new().add_filter("Imagen", &["jpg", "jpeg", "png"]).pick_file() {
                ui.global::<Backend>().set_tag_cover(f.to_string_lossy().as_ref().into());
                app.borrow_mut().tag_cover = Some(f);
            }
        });
    }
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_tag_save(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            let a = app.borrow();
            let Some(target) = a.tag_target.clone() else { return };
            let b = ui.global::<Backend>();
            let tags = media::Tags {
                title: b.get_tag_title().to_string(),
                artist: b.get_tag_artist().to_string(),
                album: b.get_tag_album().to_string(),
            };
            let cover = a.tag_cover.clone();
            b.set_tag_busy(true);
            let core = a.core.clone();
            std::thread::spawn(move || {
                let r = media::write_tags(&target, &tags, cover.as_deref());
                core.push_event(Event::TagDone(r));
            });
        });
    }

    // ---- perfil y carpeta
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_profile_changed(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            let a = app.borrow();
            let idx = ui.global::<Backend>().get_profile_index().max(0) as usize;
            if let Some(p) = a.profiles.get(idx) {
                ui.global::<Backend>().set_profile_desc(t(&p.describe()).as_str().into());
                let id = p.id.clone();
                a.core.update_settings(|s| s.active_profile_id = id.clone());
                for (i, pr) in a.profiles.iter().enumerate() {
                    if let Some(mut row) = a.profile_model.row_data(i) {
                        let active = pr.id == id;
                        if row.active != active {
                            row.active = active;
                            a.profile_model.set_row_data(i, row);
                        }
                    }
                }
            }
        });
    }
    {
        let app = app.clone();
        b.on_profile_activate(move |id| {
            let mut a = app.borrow_mut();
            let id = id.to_string();
            a.core.update_settings(|s| s.active_profile_id = id);
            a.rebuild_profiles();
        });
    }
    {
        let app = app.clone();
        b.on_profile_delete(move |id| {
            let mut a = app.borrow_mut();
            let id = id.to_string();
            a.core.delete_profile(&id);
            if a.core.settings().active_profile_id == id {
                a.core.update_settings(|s| s.active_profile_id = "audio_mp3".into());
            }
            a.rebuild_profiles();
        });
    }
    {
        let app = app.clone();
        b.on_profile_create(move |name, kind, h, c, codec| {
            let mut a = app.borrow_mut();
            let p = Profile {
                name: name.trim().to_string(),
                kind: if kind == 1 { "audio" } else { "video" }.into(),
                height: pick(&HEIGHTS, h, None),
                container: if c == 1 { "mp4" } else { "native" }.into(),
                audio_codec: pick(&CODECS, codec, "mp3").into(),
                ..Default::default()
            };
            a.core.add_profile(p);
            a.rebuild_profiles();
            a.toast("Perfil creado");
        });
    }
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_choose_folder(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            let start = app.borrow().core.settings().download_dir;
            if let Some(dir) = rfd::FileDialog::new().set_directory(&start).pick_folder() {
                let d = dir.to_string_lossy().into_owned();
                ui.global::<Backend>().set_dest_dir(d.as_str().into());
                app.borrow().core.update_settings(|s| s.download_dir = d);
            }
        });
    }

    // ---- ajustes
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_settings_changed(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            let b = ui.global::<Backend>();
            let lang = pick(&LANGS, b.get_lang_idx(), "es").to_string();
            let changed_lang = app.borrow().core.settings().language != lang;
            app.borrow().core.update_settings(|s| {
                s.concurrency = (b.get_conc_idx().max(0) + 1) as u32;
                s.rate_limit_kbps = pick(&RATES, b.get_rate_idx(), 0);
                s.cautious_mode = pick(&CAUTIOUS, b.get_cautious_idx(), "balanced").into();
                s.cookies_mode = pick(&COOKIES, b.get_cookies_mode(), "off").into();
                s.cookies_file = b.get_cookies_file().to_string();
                s.cookies_browser = pick(&BROWSERS, b.get_cookies_browser_idx(), "chrome").into();
                s.proxy_url = b.get_proxy_url().to_string();
                s.playlist_scan_limit = b.get_playlist_limit().clamp(1, 1000) as u32;
                s.sponsorblock_remove = b.get_sponsorblock();
                s.normalize_volume = b.get_normalize();
                s.embed_thumbnail_metadata = b.get_embed();
                s.download_subtitles = b.get_subs();
                s.subtitle_langs = pick(&SUB_LANGS, b.get_subs_lang_idx(), "es,en").into();
                s.auto_update_tools = b.get_auto_update();
                s.clipboard_watch = b.get_clipboard_watch();
                s.close_to_tray = b.get_close_to_tray();
                s.notifications = b.get_notifications();
                s.folder_mode = b.get_folder_idx().clamp(0, 3) as u32;
                s.name_template = b.get_name_template().to_string();
                s.language = lang.clone();
                s.compact_mode = b.get_compact_pref();
            });
            if changed_lang {
                apply_language(&lang);
                let mut a = app.borrow_mut();
                a.rebuild_profiles();
                a.rebuild_all();
            }
        });
    }
    {
        let core = core.clone();
        b.on_check_tools(move || core.check_tools(true));
    }
    {
        let ui_w = ui.as_weak();
        let core = core.clone();
        b.on_browse_cookies(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            if let Some(f) = rfd::FileDialog::new().add_filter("Cookies (txt)", &["txt"]).pick_file() {
                let p = f.to_string_lossy().into_owned();
                ui.global::<Backend>().set_cookies_file(p.as_str().into());
                core.update_settings(|s| s.cookies_file = p);
            }
        });
    }
    b.on_open_logs(|| open_in_explorer(&paths::log_dir()));
    {
        let ui_w = ui.as_weak();
        let core = core.clone();
        b.on_toggle_dark(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            let b = ui.global::<Backend>();
            let dark = !b.get_dark();
            b.set_dark(dark);
            core.update_settings(|s| s.theme = if dark { "dark" } else { "light" }.into());
        });
    }
    {
        let (app, ui_w) = (app.clone(), ui.as_weak());
        b.on_ext_choice(move |mode| {
            let Some(ui) = ui_w.upgrade() else { return };
            let b = ui.global::<Backend>();
            let url = b.get_ext_url().to_string();
            b.set_ext_dialog(false);
            let m = if mode == 1 { ExtractMode::Playlist } else { ExtractMode::Single };
            app.borrow_mut().start_extract(&ui, m, url, mode);
        });
    }
}

fn main() -> Result<(), slint::PlatformError> {
    logger::init();
    log::info!("ClipVault 2.0.0 iniciando");
    std::env::set_var("SLINT_BACKEND", "winit");

    let core = Core::new();
    let lang = core.settings().language;
    core.start();

    let ui = MainWindow::new()?;
    apply_language(&lang);
    load_settings_into_ui(&ui, &core);
    ui.global::<Backend>().set_app_version(updater::VERSION.into());

    let mut clipboard = arboard::Clipboard::new().ok();
    let last_clip = clipboard.as_mut().and_then(|c| c.get_text().ok()).unwrap_or_default().trim().to_string();
    let tray = sys::Tray::new(&t("Mostrar ClipVault"), &t("Salir"));
    let has_tray = Rc::new(Cell::new(tray.is_some()));

    let app: Shared = Rc::new(RefCell::new(App {
        core: core.clone(),
        ui: ui.as_weak(),
        cache: HashMap::new(),
        lists: Default::default(),
        list_models: Default::default(),
        trim_target: None,
        queue_model: Rc::new(VecModel::default()),
        history_model: Rc::new(VecModel::default()),
        profile_model: Rc::new(VecModel::default()),
        profiles: Vec::new(),
        toasts: VecDeque::new(),
        toast_until: Instant::now(),
        clipboard,
        last_clip,
        last_clip_check: Instant::now(),
        tray,
        #[cfg(windows)]
        taskbar: sys::win::Taskbar::new(),
        last_progress: None,
        drop_hwnd: 0,
        tag_target: None,
        tag_cover: None,
        app_update: None,
    }));
    {
        let a = app.borrow();
        let b = ui.global::<Backend>();
        b.set_search_rows(ModelRc::from(a.list_models[0].clone()));
        b.set_url_rows(ModelRc::from(a.list_models[1].clone()));
        b.set_queue_rows(ModelRc::from(a.queue_model.clone()));
        b.set_history_rows(ModelRc::from(a.history_model.clone()));
        b.set_profile_rows(ModelRc::from(a.profile_model.clone()));
    }
    app.borrow_mut().rebuild_profiles();
    wire(&ui, &app);

    // Cerrar la ventana: a la bandeja (si esta activado) o salir.
    {
        let core = core.clone();
        let has_tray = has_tray.clone();
        ui.window().on_close_requested(move || {
            if !(core.settings().close_to_tray && has_tray.get()) {
                let _ = slint::quit_event_loop();
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    let timer = slint::Timer::default();
    {
        let app = app.clone();
        timer.start(slint::TimerMode::Repeated, Duration::from_millis(120), move || {
            app.borrow_mut().tick();
        });
    }

    // `--tray` (inicio con Windows): arranca oculto en la bandeja.
    let start_hidden = has_tray.get() && std::env::args().any(|a| a == "--tray");
    if !start_hidden {
        ui.show()?;
    }
    let result = slint::run_event_loop_until_quit();
    media::stop_preview();
    core.shutdown();
    log::info!("ClipVault cerrado");
    result
}
