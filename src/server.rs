//! Servidor local para la extension de navegador: POST /api/extension/send {"url": "..."}.
use crate::core::{Core, Event};
use std::io::Read;
use std::sync::Arc;
use tiny_http::{Header, Method, Request, Response, Server};

const ADDR: &str = "127.0.0.1:5000";

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("cabecera valida")
}

fn reply(req: Request, status: u16, body: &str, origin: Option<&str>) {
    let mut r = Response::from_string(body).with_status_code(status).with_header(header("Content-Type", "application/json"));
    if let Some(o) = origin {
        r = r
            .with_header(header("Access-Control-Allow-Origin", o))
            .with_header(header("Access-Control-Allow-Methods", "GET, POST, OPTIONS"))
            .with_header(header("Access-Control-Allow-Headers", "Content-Type"))
            .with_header(header("Vary", "Origin"));
    }
    let _ = req.respond(r);
}

pub fn start(core: Arc<Core>) {
    std::thread::spawn(move || {
        let server = match Server::http(ADDR) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("No se pudo abrir {ADDR}: {e}");
                core.toast("Puerto 5000 ocupado (¿otra copia de ClipVault abierta?). La extensión no funcionará.");
                return;
            }
        };
        log::info!("Servidor de extensión escuchando en {ADDR}");
        for mut req in server.incoming_requests() {
            // Solo extensiones de navegador pueden hablar con la app (evita que una web cualquiera envie enlaces).
            let origin = req
                .headers()
                .iter()
                .find(|h| h.field.equiv("Origin"))
                .map(|h| h.value.as_str().to_string());
            let allowed = origin.as_deref().filter(|o| o.starts_with("chrome-extension://") || o.starts_with("moz-extension://"));
            if origin.is_some() && allowed.is_none() {
                reply(req, 403, r#"{"error":"origen no permitido"}"#, None);
                continue;
            }
            match (req.method().clone(), req.url().split('?').next().unwrap_or("")) {
                (Method::Options, _) => reply(req, 204, "", allowed),
                (Method::Get, "/api/ping") => reply(req, 200, r#"{"ok":true,"app":"ClipVault"}"#, allowed),
                (Method::Post, "/api/extension/send") => {
                    let mut body = String::new();
                    let _ = req.as_reader().take(65_536).read_to_string(&mut body);
                    let json = serde_json::from_str::<serde_json::Value>(&body).ok();
                    let mode = json.as_ref().and_then(|v| match v["mode"].as_str() {
                        Some("single") => Some(0u8),
                        Some("playlist") => Some(1u8),
                        _ => None,
                    });
                    let url = json
                        .and_then(|v| v["url"].as_str().map(|s| s.trim().to_string()))
                        .filter(|u| u.starts_with("http://") || u.starts_with("https://"));
                    match url {
                        Some(u) => {
                            core.push_event(Event::ExtLink { url: u, mode });
                            reply(req, 200, r#"{"ok":true}"#, allowed);
                        }
                        None => reply(req, 400, r#"{"error":"URL vacía o inválida"}"#, allowed),
                    }
                }
                _ => reply(req, 404, r#"{"error":"no encontrado"}"#, allowed),
            }
        }
    });
}
