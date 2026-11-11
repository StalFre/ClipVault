//! Plantillas de nombre, organizacion en carpetas y rangos de recorte.
use crate::model::{Profile, Video};
use crate::util::sanitize_filename;
use std::path::{Path, PathBuf};

pub const DEFAULT_TEMPLATE: &str = "{title}";

pub fn render_name(template: &str, v: &Video, p: &Profile) -> String {
    let t = if template.trim().is_empty() { DEFAULT_TEMPLATE } else { template };
    let channel = if v.channel.is_empty() { "Sin canal" } else { v.channel.as_str() };
    let out = t
        .replace("{title}", &v.title)
        .replace("{channel}", channel)
        .replace("{id}", &v.id)
        .replace("{profile}", &p.name)
        .replace("{date}", &chrono::Local::now().format("%Y-%m-%d").to_string());
    sanitize_filename(&out)
}

/// 0 = Perfil/Lista, 1 = Canal, 2 = Perfil/Canal, 3 = sin subcarpetas
pub fn dest_for(base: &Path, mode: u32, p: &Profile, v: &Video, playlist: &str) -> PathBuf {
    let mut d = base.to_path_buf();
    let channel = sanitize_filename(if v.channel.is_empty() { "Sin canal" } else { &v.channel });
    match mode {
        1 => d.push(channel),
        2 => {
            d.push(sanitize_filename(&p.name));
            d.push(channel);
        }
        3 => {}
        _ => {
            d.push(sanitize_filename(&p.name));
            if !playlist.trim().is_empty() {
                d.push(sanitize_filename(playlist));
            }
        }
    }
    d
}

/// "90", "1:30" o "1:02:03" -> segundos.
pub fn parse_time(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.trim().split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut total = 0u32;
    for p in parts {
        total = total.checked_mul(60)?.checked_add(p.trim().parse::<u32>().ok()?)?;
    }
    Some(total)
}

/// Devuelve el argumento `--download-sections` o None si no hay rango.
pub fn section_arg(from: &str, to: &str) -> Result<Option<String>, String> {
    let (f, t) = (from.trim(), to.trim());
    if f.is_empty() && t.is_empty() {
        return Ok(None);
    }
    let a = if f.is_empty() { 0 } else { parse_time(f).ok_or("Tiempo inicial inválido (usa mm:ss)")? };
    let b = if t.is_empty() { None } else { Some(parse_time(t).ok_or("Tiempo final inválido (usa mm:ss)")?) };
    if let Some(b) = b {
        if b <= a {
            return Err("El tiempo final debe ser mayor que el inicial".into());
        }
    }
    Ok(Some(match b {
        Some(b) => format!("*{a}-{b}"),
        None => format!("*{a}-inf"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(parse_time("1:30"), Some(90));
        assert_eq!(parse_time("1:02:03"), Some(3723));
        assert_eq!(parse_time("x"), None);
        assert_eq!(section_arg("1:00", "2:00").unwrap(), Some("*60-120".into()));
        assert_eq!(section_arg("", "").unwrap(), None);
        assert!(section_arg("2:00", "1:00").is_err());
    }

    #[test]
    fn names() {
        let v = Video { title: "A:B".into(), channel: "Canal".into(), id: "x".into(), ..Default::default() };
        let p = Profile::default();
        assert_eq!(render_name("{channel} - {title}", &v, &p), "Canal - A_B");
    }
}
