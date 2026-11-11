//! Estado compartido: cola de descargas, historial, perfiles y eventos hacia la UI.
use crate::config::{self, Settings};
use crate::history::{self, HistoryRecord};
use crate::model::{Extracted, Profile, Skipped, Video};
use crate::thumbs::Thumbs;
use crate::tools::{self, ToolsInfo};
use crate::{args, extractor, naming, notify, server, updater, util};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

const MIN_FREE_BYTES: u64 = 200 * 1024 * 1024;
const MAX_ATTEMPTS: u32 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Queued,
    Downloading,
    Paused,
    Cancelling,
}

pub struct Item {
    pub id: u64,
    pub job: u64,
    pub video: Video,
    pub profile: Profile,
    pub dest_dir: String,
    pub status: Status,
    pub pct: f32,
    pub speed: String,
    pub eta: String,
    pub stage: String,
    pub stem: String,
    pub range: Option<String>,
    running: bool,
    stop: bool,
    pid: Option<u32>,
}

pub enum Event {
    Toast(String),
    ExtLink { url: String, mode: Option<u8> },
    Dropped(Vec<String>),
    TagDone(Result<(), String>),
    AppUpdate { result: Result<Option<updater::UpdateInfo>, String>, manual: bool },
    Quit,
    TrimFrame(Result<(Vec<u8>, u32, u32), String>),
    Extracted { list: usize, result: Result<Extracted, String> },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExtractMode {
    Search(u32),
    Single,
    Playlist,
    Bulk,
}

enum Outcome {
    Ok,
    Stopped,
    Failed(String),
}

pub struct Core {
    pub settings: Mutex<Settings>,
    pub custom_profiles: Mutex<Vec<Profile>>,
    pub items: Mutex<Vec<Item>>,
    pub history: Mutex<Vec<HistoryRecord>>,
    pub queue_dirty: AtomicBool,
    pub history_dirty: AtomicBool,
    pub events: Mutex<VecDeque<Event>>,
    pub tools: Mutex<ToolsInfo>,
    pub thumbs: Thumbs,
    next_id: AtomicU64,
    jobs: Mutex<HashMap<u64, (usize, usize)>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Core {
    pub fn new() -> Arc<Core> {
        Arc::new(Core {
            settings: Mutex::new(config::load_settings()),
            custom_profiles: Mutex::new(config::load_custom_profiles()),
            items: Mutex::new(Vec::new()),
            history: Mutex::new(history::load()),
            queue_dirty: AtomicBool::new(true),
            history_dirty: AtomicBool::new(true),
            events: Mutex::new(VecDeque::new()),
            tools: Mutex::new(ToolsInfo { status: "Comprobando herramientas…".into(), busy: true, progress: -1.0, ..Default::default() }),
            thumbs: Thumbs::new(),
            next_id: AtomicU64::new(1),
            jobs: Mutex::new(HashMap::new()),
        })
    }

    /// Arranca dispatcher, servidor de la extension y comprobacion de herramientas.
    pub fn start(self: &Arc<Self>) {
        let c = self.clone();
        std::thread::spawn(move || c.dispatch_loop());
        server::start(self.clone());
        self.check_tools(false);
        if self.settings().auto_update_tools {
            let c = self.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(6));
                c.check_app_update(false);
            });
        }
    }

    pub fn check_app_update(self: &Arc<Self>, manual: bool) {
        let c = self.clone();
        std::thread::spawn(move || {
            let result = updater::check();
            if let Err(e) = &result {
                log::warn!("comprobar actualización de la app: {e}");
            }
            c.push_event(Event::AppUpdate { result, manual });
        });
    }

    /// Descarga el instalador, lo lanza y cierra la app para que pueda reemplazarla.
    pub fn install_app_update(self: &Arc<Self>, info: updater::UpdateInfo) {
        self.toast("Descargando actualización…");
        let c = self.clone();
        std::thread::spawn(move || match updater::download_and_run(&info) {
            Ok(()) => {
                std::thread::sleep(Duration::from_secs(2));
                c.push_event(Event::Quit);
            }
            Err(e) => c.toast(&format!("No se pudo actualizar: {e}")),
        });
    }

    /// `manual`: el usuario pidio comprobar; siempre actualiza lo desactualizado.
    pub fn check_tools(self: &Arc<Self>, manual: bool) {
        if lock(&self.tools).busy && manual {
            return;
        }
        if manual && lock(&self.items).iter().any(|i| i.running) {
            self.toast("Hay descargas activas: actualiza cuando terminen");
            return;
        }
        let c = self.clone();
        std::thread::spawn(move || {
            let auto = lock(&c.settings).auto_update_tools;
            tools::check_and_update(&c.tools, manual || auto);
            let t = lock(&c.tools).clone();
            if manual {
                c.toast(&t.status);
            }
        });
    }

    pub fn toast(&self, msg: &str) {
        lock(&self.events).push_back(Event::Toast(msg.to_string()));
    }

    pub fn push_event(&self, e: Event) {
        lock(&self.events).push_back(e);
    }

    pub fn take_events(&self) -> Vec<Event> {
        lock(&self.events).drain(..).collect()
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        let mut s = lock(&self.settings);
        f(&mut s);
        s.concurrency = s.concurrency.clamp(1, 4);
        config::save_settings(&s);
    }

    pub fn profiles(&self) -> Vec<Profile> {
        config::all_profiles(&lock(&self.custom_profiles))
    }

    pub fn add_profile(&self, mut p: Profile) {
        p.builtin = false;
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        p.id = format!("custom_{ms}");
        let mut c = lock(&self.custom_profiles);
        c.push(p);
        config::save_custom_profiles(&c);
    }

    pub fn delete_profile(&self, id: &str) {
        let mut c = lock(&self.custom_profiles);
        c.retain(|p| p.id != id);
        config::save_custom_profiles(&c);
    }

    /// Al cerrar la app: mata los procesos de descarga que sigan vivos.
    pub fn shutdown(&self) {
        for it in lock(&self.items).iter() {
            if let Some(p) = it.pid {
                util::kill_tree(p);
            }
        }
    }

    // ------------------------------------------------------------------ extraccion

    pub fn extract_async(self: &Arc<Self>, list: usize, mode: ExtractMode, input: String) {
        let c = self.clone();
        std::thread::spawn(move || {
            let s = c.settings();
            let result = match mode {
                ExtractMode::Search(n) => extractor::extract(&format!("ytsearch{n}:{}", input.trim()), &s, false),
                ExtractMode::Single | ExtractMode::Playlist => {
                    let url = input.trim();
                    if !(url.starts_with("http://") || url.starts_with("https://")) {
                        Err("Pega un enlace válido que empiece con http:// o https://".to_string())
                    } else {
                        extractor::extract(url, &s, mode == ExtractMode::Single)
                    }
                }
                ExtractMode::Bulk => {
                    let mut all = Extracted::default();
                    for url in input.lines().map(str::trim).filter(|l| !l.is_empty()) {
                        if !url.starts_with("http") {
                            all.skipped.push(Skipped { title: url.to_string(), reason: "No es un enlace válido".into() });
                            continue;
                        }
                        match extractor::extract(url, &s, true) {
                            Ok(mut e) => {
                                all.items.append(&mut e.items);
                                all.skipped.append(&mut e.skipped);
                            }
                            Err(e) => all.skipped.push(Skipped { title: url.to_string(), reason: e }),
                        }
                    }
                    Ok(all)
                }
            };
            c.push_event(Event::Extracted { list, result });
        });
    }

    // ------------------------------------------------------------------ cola

    /// Encola videos. `dest_override` fuerza la carpeta (reintentos); si no, se organiza segun los ajustes.
    /// yt-dlp omite por si solo archivos que ya existen en disco.
    pub fn enqueue(&self, videos: &[Video], profile: &Profile, dest_override: Option<PathBuf>, ranges: &[Option<String>]) -> usize {
        let st = self.settings();
        let base = PathBuf::from(&st.download_dir);
        let job = self.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.jobs).insert(job, (videos.len(), 0));
        for (n, v) in videos.iter().enumerate() {
            let range = ranges.get(n).cloned().flatten();
            let dest = dest_override.clone().unwrap_or_else(|| naming::dest_for(&base, st.folder_mode, profile, v, &v.playlist));
            let _ = std::fs::create_dir_all(&dest);
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            lock(&self.items).push(Item {
                id,
                job,
                video: v.clone(),
                profile: profile.clone(),
                dest_dir: dest.to_string_lossy().into_owned(),
                status: Status::Queued,
                pct: 0.0,
                speed: String::new(),
                eta: String::new(),
                stage: "En cola".into(),
                stem: naming::render_name(&st.name_template, v, profile),
                range,
                running: false,
                stop: false,
                pid: None,
            });
        }
        log::info!("Job {job}: {} en cola (perfil={})", videos.len(), profile.name);
        self.queue_dirty.store(true, Ordering::Relaxed);
        videos.len()
    }

    pub fn add_history(&self, v: &Video, p: &Profile, dest: &std::path::Path, status: &str, error: Option<String>, file: String) {
        let rec = HistoryRecord {
            item_id: self.next_id.fetch_add(1, Ordering::Relaxed).to_string(),
            title: v.title.clone(),
            url: v.url.clone(),
            video: v.clone(),
            profile: p.clone(),
            status: status.into(),
            error,
            dest_dir: dest.to_string_lossy().into_owned(),
            finished_at: util::now_stamp(),
            file,
        };
        let mut h = lock(&self.history);
        h.insert(0, rec);
        h.truncate(history::MAX_RECORDS);
        history::save(&h);
        self.history_dirty.store(true, Ordering::Relaxed);
    }

    pub fn clear_history(&self) {
        let mut h = lock(&self.history);
        h.clear();
        history::save(&h);
        self.history_dirty.store(true, Ordering::Relaxed);
    }

    pub fn retry(&self, item_id: &str) {
        let rec = {
            let mut h = lock(&self.history);
            let Some(pos) = h.iter().position(|r| r.item_id == item_id) else { return };
            let rec = h.remove(pos);
            history::save(&h);
            rec
        };
        self.history_dirty.store(true, Ordering::Relaxed);
        let dest = (!rec.dest_dir.is_empty()).then(|| PathBuf::from(&rec.dest_dir));
        self.enqueue(&[rec.video], &rec.profile, dest, &[]);
    }

    fn job_progress(&self, job: u64) {
        let finished = {
            let mut jobs = lock(&self.jobs);
            match jobs.get_mut(&job) {
                Some(e) => {
                    e.1 += 1;
                    (e.1 >= e.0).then_some(e.0)
                }
                None => None,
            }
        };
        if let Some(total) = finished {
            lock(&self.jobs).remove(&job);
            self.toast(&format!("Descarga completa · {total} elemento(s)"));
            if self.settings().notifications {
                notify::toast("ClipVault", &format!("Descarga completa · {total} elemento(s)"));
            }
        }
    }

    fn mark_dirty(&self) {
        self.queue_dirty.store(true, Ordering::Relaxed);
    }

    fn update(&self, id: u64, f: impl FnOnce(&mut Item)) {
        if let Some(it) = lock(&self.items).iter_mut().find(|i| i.id == id) {
            f(it);
            self.mark_dirty();
        }
    }

    pub fn pause(&self, id: u64) {
        let pid = {
            let mut items = lock(&self.items);
            let Some(it) = items.iter_mut().find(|i| i.id == id) else { return };
            match it.status {
                Status::Queued => {
                    it.status = Status::Paused;
                    None
                }
                Status::Downloading => {
                    it.status = Status::Paused;
                    it.stop = true;
                    it.stage = "En pausa".into();
                    it.pid
                }
                _ => return,
            }
        };
        self.mark_dirty();
        if let Some(p) = pid {
            util::kill_tree(p);
        }
    }

    pub fn resume(&self, id: u64) {
        let mut items = lock(&self.items);
        if let Some(it) = items.iter_mut().find(|i| i.id == id && i.status == Status::Paused) {
            it.status = Status::Queued;
            it.stage = "En cola".into();
            drop(items);
            self.mark_dirty();
        }
    }

    pub fn cancel(&self, id: u64) {
        let (pid, job_done) = {
            let mut items = lock(&self.items);
            let Some(pos) = items.iter().position(|i| i.id == id) else { return };
            if items[pos].running {
                let it = &mut items[pos];
                if it.status == Status::Cancelling {
                    return;
                }
                it.status = Status::Cancelling;
                it.stop = true;
                it.stage = "Cancelando…".into();
                (it.pid, None)
            } else {
                let it = items.remove(pos);
                (None, Some(it.job))
            }
        };
        self.mark_dirty();
        if let Some(j) = job_done {
            self.job_progress(j);
        }
        if let Some(p) = pid {
            util::kill_tree(p);
        }
    }

    pub fn clear_queue(&self) -> usize {
        let ids: Vec<u64> = lock(&self.items).iter().map(|i| i.id).collect();
        for id in &ids {
            self.cancel(*id);
        }
        ids.len()
    }

    /// Mueve un elemento `delta` posiciones (negativo = hacia arriba).
    pub fn move_item(&self, id: u64, delta: i32) {
        let mut items = lock(&self.items);
        if let Some(pos) = items.iter().position(|i| i.id == id) {
            let to = (pos as i32 + delta).clamp(0, items.len() as i32 - 1) as usize;
            if to != pos {
                let it = items.remove(pos);
                items.insert(to, it);
                drop(items);
                self.mark_dirty();
            }
        }
    }

    pub fn item_dest(&self, id: u64) -> Option<String> {
        lock(&self.items).iter().find(|i| i.id == id).map(|i| i.dest_dir.clone())
    }

    // ------------------------------------------------------------------ ejecucion

    fn dispatch_loop(self: Arc<Self>) {
        loop {
            std::thread::sleep(Duration::from_millis(300));
            if lock(&self.tools).busy || !tools::ready() {
                continue;
            }
            let conc = lock(&self.settings).concurrency.max(1) as usize;
            let mut start = Vec::new();
            {
                let mut items = lock(&self.items);
                let running = items.iter().filter(|i| i.running).count();
                let mut free = conc.saturating_sub(running);
                for it in items.iter_mut() {
                    if free == 0 {
                        break;
                    }
                    if it.status == Status::Queued && !it.running {
                        it.running = true;
                        it.stop = false;
                        it.status = Status::Downloading;
                        it.stage = "Preparando…".into();
                        it.pct = 0.0;
                        free -= 1;
                        start.push(it.id);
                    }
                }
            }
            if !start.is_empty() {
                self.mark_dirty();
            }
            for id in start {
                let c = self.clone();
                std::thread::spawn(move || c.run_item(id));
            }
        }
    }

    fn stop_requested(&self, id: u64) -> bool {
        lock(&self.items).iter().find(|i| i.id == id).map_or(true, |i| i.stop)
    }

    fn run_item(self: &Arc<Self>, id: u64) {
        let Some((video, profile, dest_dir, job, safe, range)) = lock(&self.items)
            .iter()
            .find(|i| i.id == id)
            .map(|i| (i.video.clone(), i.profile.clone(), i.dest_dir.clone(), i.job, i.stem.clone(), i.range.clone()))
        else {
            return;
        };
        let settings = self.settings();
        let dest = PathBuf::from(&dest_dir);
        let _ = std::fs::create_dir_all(&dest);

        let mut success = false;
        let mut stopped = false;
        let mut last_error = String::new();

        if util::free_space(&dest).map_or(false, |f| f < MIN_FREE_BYTES) {
            last_error = "Espacio en disco insuficiente (menos de 200 MB libres)".into();
        } else {
            let template = format!("{}.%(ext)s", dest.join(&safe).to_string_lossy().replace('%', "%%"));
            let ff = tools::ffmpeg_dir().map(|p| p.to_string_lossy().into_owned());
            let argv = args::download_args(&profile, &settings, &template, &video.url, ff.as_deref(), range.as_deref());
            for attempt in 1..=MAX_ATTEMPTS {
                if self.stop_requested(id) {
                    stopped = true;
                    break;
                }
                self.update(id, |it| {
                    it.stage = if attempt > 1 { format!("Reintentando ({attempt}/{MAX_ATTEMPTS})…") } else { "Preparando…".into() };
                });
                match self.run_process(id, &argv) {
                    Outcome::Ok => {
                        success = true;
                        break;
                    }
                    Outcome::Stopped => {
                        stopped = true;
                        break;
                    }
                    Outcome::Failed(msg) => {
                        last_error = msg;
                        if util::is_permanent_error(&last_error) || attempt == MAX_ATTEMPTS {
                            break;
                        }
                        for _ in 0..(15 * attempt) {
                            if self.stop_requested(id) {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            }
            if success {
                if let Some(ext) = args::expected_audio_ext(&profile) {
                    if !dest.join(format!("{safe}.{ext}")).is_file() {
                        success = false;
                        last_error = format!("No se generó el archivo .{ext}. Revisa que ffmpeg funcione.");
                    }
                }
            }
        }

        if success {
            self.remove_item(id);
            let ext = args::expected_audio_ext(&profile).unwrap_or("mp4");
            let f = dest.join(format!("{safe}.{ext}"));
            let file = if f.is_file() { f.to_string_lossy().into_owned() } else { String::new() };
            self.add_history(&video, &profile, &dest, "done", None, file);
            log::info!("Descarga OK: {}", video.title);
            self.job_progress(job);
        } else if stopped {
            let mut cancelled_job = None;
            {
                let mut items = lock(&self.items);
                if let Some(pos) = items.iter().position(|i| i.id == id) {
                    let it = &mut items[pos];
                    it.running = false;
                    it.stop = false;
                    it.pid = None;
                    match it.status {
                        Status::Cancelling => cancelled_job = Some(items.remove(pos).job),
                        Status::Downloading => it.status = Status::Queued,
                        Status::Paused | Status::Queued => {}
                    }
                }
            }
            self.mark_dirty();
            if let Some(j) = cancelled_job {
                self.job_progress(j);
            }
        } else {
            log::error!("Descarga fallida: {} -- {last_error}", video.title);
            self.remove_item(id);
            self.add_history(&video, &profile, &dest, "failed", Some(last_error), String::new());
            self.toast(&format!("Falló: {}", video.title));
            self.job_progress(job);
        }
    }

    fn remove_item(&self, id: u64) {
        lock(&self.items).retain(|i| i.id != id);
        self.mark_dirty();
    }

    fn run_process(self: &Arc<Self>, id: u64, argv: &[String]) -> Outcome {
        let mut child = match util::command(tools::ytdlp_path())
            .args(argv)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return Outcome::Failed(format!("No se pudo ejecutar yt-dlp: {e}")),
        };
        let pid = child.id();
        let stop_now = {
            let mut items = lock(&self.items);
            match items.iter_mut().find(|i| i.id == id) {
                Some(it) => {
                    it.pid = Some(pid);
                    it.stop
                }
                None => true,
            }
        };
        if stop_now {
            util::kill_tree(pid);
        }

        let (tx, rx) = channel::<String>();
        for reader in [
            child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
            child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let tx = tx.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(reader).split(b'\n').map_while(Result::ok) {
                    let _ = tx.send(String::from_utf8_lossy(&line).trim_end_matches('\r').to_string());
                }
            });
        }
        drop(tx);

        let mut tail: Vec<String> = Vec::new();
        for line in rx {
            if !self.apply_line(id, &line) && !line.trim().is_empty() {
                tail.push(line);
                if tail.len() > 40 {
                    tail.remove(0);
                }
            }
        }
        let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
        if self.stop_requested(id) {
            return Outcome::Stopped;
        }
        if code == 0 {
            return Outcome::Ok;
        }
        let msg = tail
            .iter()
            .rev()
            .find(|l| l.contains("ERROR"))
            .cloned()
            .or_else(|| tail.last().cloned())
            .map(|l| l.trim().trim_start_matches("ERROR:").trim().to_string())
            .unwrap_or_else(|| format!("yt-dlp terminó con código {code}"));
        Outcome::Failed(msg)
    }

    /// Interpreta una linea de yt-dlp. Devuelve true si era de progreso/estado.
    fn apply_line(&self, id: u64, line: &str) -> bool {
        if let Some((pct, speed, eta)) = parse_progress(line) {
            self.update(id, |it| {
                it.pct = pct;
                it.speed = speed;
                it.eta = eta;
                it.stage = "Descargando".into();
            });
            return true;
        }
        let stage = if line.starts_with("[Merger]") {
            "Uniendo audio y video…"
        } else if line.starts_with("[ExtractAudio]") || line.starts_with("[VideoConvertor]") || line.starts_with("[FixupM4a]") {
            "Convirtiendo…"
        } else if line.starts_with("[EmbedThumbnail]") || line.starts_with("[Metadata]") || line.starts_with("[ThumbnailsConvertor]") {
            "Incrustando metadatos…"
        } else if line.starts_with("[SponsorBlock]") || line.starts_with("[ModifyChapters]") {
            "Quitando patrocinios…"
        } else if line.starts_with("[download] Destination") {
            "Descargando"
        } else if line.starts_with('[') && !line.contains("ERROR") {
            return true;
        } else {
            return false;
        };
        let post = !stage.starts_with("Descargando");
        self.update(id, |it| {
            it.stage = stage.into();
            if post {
                it.pct = 100.0;
                it.speed.clear();
                it.eta.clear();
            }
        });
        true
    }
}

/// "[download]  45.3% of 10.00MiB at 2.10MiB/s ETA 00:05" -> (45.3, "2.10MiB/s", "00:05")
pub fn parse_progress(line: &str) -> Option<(f32, String, String)> {
    let rest = line.strip_prefix("[download]")?;
    let pct_idx = rest.find('%')?;
    let pct: f32 = rest[..pct_idx].split_whitespace().last()?.parse().ok()?;
    let mut speed = String::new();
    let mut eta = String::new();
    let words: Vec<&str> = rest[pct_idx..].split_whitespace().collect();
    for (i, w) in words.iter().enumerate() {
        match *w {
            "at" => speed = words.get(i + 1).map(|s| s.to_string()).unwrap_or_default(),
            "ETA" => eta = words.get(i + 1).map(|s| s.to_string()).unwrap_or_default(),
            _ => {}
        }
    }
    if speed == "Unknown" {
        speed.clear();
    }
    if eta == "Unknown" {
        eta.clear();
    }
    Some((pct.clamp(0.0, 100.0), speed, eta))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_line() {
        let (p, s, e) = parse_progress("[download]  45.3% of   10.00MiB at    2.10MiB/s ETA 00:05").unwrap();
        assert_eq!(p, 45.3);
        assert_eq!(s, "2.10MiB/s");
        assert_eq!(e, "00:05");
        assert!(parse_progress("[download] Destination: x.mp3").is_none());
        let (p, s, e) = parse_progress("[download] 100% of 3.00MiB in 00:00:01 at 2.9MiB/s").unwrap();
        assert_eq!((p, s.as_str(), e.as_str()), (100.0, "2.9MiB/s", ""));
    }

    #[test]
    #[ignore]
    fn live_download_mp3() {
        let core = Core::new();
        core.start();
        while crate::tools::ready() == false || core.tools.lock().unwrap().busy {
            std::thread::sleep(Duration::from_millis(500));
        }
        let ex = crate::extractor::extract("ytsearch2:me at the zoo", &core.settings(), false).unwrap();
        println!("{:?}", ex.items);
        assert!(!ex.items.is_empty());
        let p = crate::model::builtin_profiles().into_iter().find(|p| p.id == "audio_mp3").unwrap();
        let dest = std::env::temp_dir().join("cv_test_out");
        let _ = std::fs::remove_dir_all(&dest);
        core.enqueue(&ex.items[..1], &p, Some(dest.clone()), &[]);
        for _ in 0..240 {
            std::thread::sleep(Duration::from_millis(500));
            if core.items.lock().unwrap().is_empty() { break; }
        }
        let files: Vec<_> = std::fs::read_dir(&dest).unwrap().map(|e| e.unwrap().file_name()).collect();
        println!("{files:?} hist={:?}", core.history.lock().unwrap().first().map(|h| (h.status.clone(), h.error.clone())));
        assert!(files.iter().any(|f| f.to_string_lossy().ends_with(".mp3")));
    }
}
