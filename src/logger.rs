//! Log a archivo con rotacion por tamano (equivalente a RotatingFileHandler).
use log::{LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_BYTES: u64 = 1_000_000;
const BACKUPS: u32 = 5;

struct State {
    file: Option<File>,
    size: u64,
}

struct RotLogger {
    path: PathBuf,
    state: Mutex<State>,
}

impl RotLogger {
    fn open(path: &PathBuf) -> (Option<File>, u64) {
        let f = OpenOptions::new().create(true).append(true).open(path).ok();
        let size = f.as_ref().and_then(|f| f.metadata().ok()).map(|m| m.len()).unwrap_or(0);
        (f, size)
    }

    fn rotate(&self, st: &mut State) {
        st.file = None; // Windows no permite renombrar un archivo abierto
        let name = |n: u32| PathBuf::from(format!("{}.{}", self.path.display(), n));
        let _ = fs::remove_file(name(BACKUPS));
        for n in (1..BACKUPS).rev() {
            let _ = fs::rename(name(n), name(n + 1));
        }
        let _ = fs::rename(&self.path, name(1));
        let (f, s) = Self::open(&self.path);
        st.file = f;
        st.size = s;
    }
}

impl Log for RotLogger {
    fn enabled(&self, m: &Metadata) -> bool {
        m.target().starts_with("clipvault") || m.level() <= log::Level::Warn
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!(
            "{} [{}] {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            r.level(),
            r.args()
        );
        if let Ok(mut st) = self.state.lock() {
            if st.size + line.len() as u64 > MAX_BYTES {
                self.rotate(&mut st);
            }
            if let Some(f) = st.file.as_mut() {
                if f.write_all(line.as_bytes()).is_ok() {
                    st.size += line.len() as u64;
                }
            }
        }
    }

    fn flush(&self) {}
}

pub fn init() {
    let path = crate::paths::log_dir().join("clipvault.log");
    let (file, size) = RotLogger::open(&path);
    let logger = RotLogger { path, state: Mutex::new(State { file, size }) };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
    std::panic::set_hook(Box::new(|info| {
        log::error!("PANIC: {info}");
    }));
}
