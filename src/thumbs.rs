//! Descarga y decodifica miniaturas en un pool de hilos. La UI recoge los pixeles ya listos.
use std::collections::HashSet;
use std::io::Read;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const THUMB_W: u32 = 224;
pub const THUMB_H: u32 = 126;

pub struct Decoded {
    pub url: String,
    /// None = fallo (la UI muestra el placeholder y no reintenta)
    pub rgba: Option<Vec<u8>>,
}

pub struct Thumbs {
    tx: Sender<String>,
    seen: Mutex<HashSet<String>>,
    ready: Arc<Mutex<Vec<Decoded>>>,
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout_read(Duration::from_secs(12))
        .user_agent("Mozilla/5.0 ClipVault/2.0")
        .build();
    let resp = agent.get(url).call().map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    resp.into_reader().take(8 * 1024 * 1024).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let img = image::load_from_memory(&buf).map_err(|e| e.to_string())?;
    let img = img.resize_to_fill(THUMB_W, THUMB_H, image::imageops::FilterType::Triangle);
    Ok(img.to_rgba8().into_raw())
}

impl Thumbs {
    pub fn new() -> Thumbs {
        let (tx, rx) = channel::<String>();
        let rx = Arc::new(Mutex::new(rx));
        let ready: Arc<Mutex<Vec<Decoded>>> = Arc::new(Mutex::new(Vec::new()));
        for _ in 0..4 {
            let (rx, ready) = (rx.clone(), ready.clone());
            std::thread::spawn(move || loop {
                let url = match rx.lock().map(|r| r.recv()) {
                    Ok(Ok(u)) => u,
                    _ => return,
                };
                let rgba = match fetch(&url) {
                    Ok(b) => Some(b),
                    Err(e) => {
                        log::warn!("miniatura fallida {url}: {e}");
                        None
                    }
                };
                if let Ok(mut r) = ready.lock() {
                    r.push(Decoded { url, rgba });
                }
            });
        }
        Thumbs { tx, seen: Mutex::new(HashSet::new()), ready }
    }

    /// Pide una miniatura (una sola vez por URL).
    pub fn request(&self, url: &str) {
        if url.is_empty() {
            return;
        }
        if self.seen.lock().map(|mut s| s.insert(url.to_string())).unwrap_or(false) {
            let _ = self.tx.send(url.to_string());
        }
    }

    pub fn drain(&self) -> Vec<Decoded> {
        self.ready.lock().map(|mut r| std::mem::take(&mut *r)).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn live_thumb() {
        let px = super::fetch("https://i.ytimg.com/vi/jNQXAC9IVRw/mqdefault.jpg").unwrap();
        assert_eq!(px.len(), (super::THUMB_W * super::THUMB_H * 4) as usize);
    }
}
