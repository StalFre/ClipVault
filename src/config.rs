use crate::model::{builtin_profiles, Profile};
use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub download_dir: String,
    pub cookies_mode: String,
    pub cookies_file: String,
    pub cookies_browser: String,
    pub proxy_url: String,
    pub playlist_scan_limit: u32,
    pub cautious_mode: String,
    pub active_profile_id: String,
    pub rate_limit_kbps: u32,
    pub concurrency: u32,
    pub normalize_volume: bool,
    pub sponsorblock_remove: bool,
    pub embed_thumbnail_metadata: bool,
    pub download_subtitles: bool,
    pub subtitle_langs: String,
    pub auto_update_tools: bool,
    pub clipboard_watch: bool,
    pub close_to_tray: bool,
    pub notifications: bool,
    /// 0 = Perfil/Lista, 1 = Canal, 2 = Perfil/Canal, 3 = sin subcarpetas
    pub folder_mode: u32,
    pub name_template: String,
    pub language: String,
    pub compact_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "dark".into(),
            download_dir: paths::default_download_dir().to_string_lossy().into_owned(),
            cookies_mode: "off".into(),
            cookies_file: String::new(),
            cookies_browser: "chrome".into(),
            proxy_url: String::new(),
            playlist_scan_limit: 300,
            cautious_mode: "balanced".into(),
            active_profile_id: "audio_mp3".into(),
            rate_limit_kbps: 0,
            concurrency: 2,
            normalize_volume: false,
            sponsorblock_remove: false,
            embed_thumbnail_metadata: false,
            download_subtitles: false,
            subtitle_langs: "es,en".into(),
            auto_update_tools: true,
            clipboard_watch: true,
            close_to_tray: true,
            notifications: true,
            folder_mode: 0,
            name_template: "{title}".into(),
            language: "es".into(),
            compact_mode: false,
        }
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            log::warn!("JSON corrupto en {}: {e}; se renombra a .bad", path.display());
            let _ = std::fs::rename(path, path.with_extension("json.bad"));
            None
        }
    }
}

/// Escritura atomica: archivo temporal + rename.
pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) {
    let tmp = path.with_extension("json.tmp");
    let res = serde_json::to_string_pretty(value)
        .map_err(|e| e.to_string())
        .and_then(|s| std::fs::write(&tmp, s).map_err(|e| e.to_string()))
        .and_then(|_| std::fs::rename(&tmp, path).map_err(|e| e.to_string()));
    if let Err(e) = res {
        log::error!("No se pudo guardar {}: {e}", path.display());
    }
}

fn settings_path() -> PathBuf {
    paths::data_dir().join("settings.json")
}
fn profiles_path() -> PathBuf {
    paths::data_dir().join("profiles.json")
}

pub fn load_settings() -> Settings {
    let mut s: Settings = read_json(&settings_path()).unwrap_or_default();
    s.concurrency = s.concurrency.clamp(1, 4);
    if s.download_dir.trim().is_empty() {
        s.download_dir = paths::default_download_dir().to_string_lossy().into_owned();
    }
    s
}

pub fn save_settings(s: &Settings) {
    write_json(&settings_path(), s);
}

#[derive(Serialize, Deserialize, Default)]
struct ProfilesFile {
    profiles: Vec<Profile>,
}

/// Solo se guardan los perfiles personalizados; los integrados viven en codigo.
pub fn load_custom_profiles() -> Vec<Profile> {
    read_json::<ProfilesFile>(&profiles_path())
        .map(|f| f.profiles.into_iter().filter(|p| !p.builtin && !p.id.is_empty()).collect())
        .unwrap_or_default()
}

pub fn save_custom_profiles(p: &[Profile]) {
    write_json(&profiles_path(), &ProfilesFile { profiles: p.to_vec() });
}

pub fn all_profiles(custom: &[Profile]) -> Vec<Profile> {
    let mut v = builtin_profiles();
    v.extend(custom.iter().cloned());
    v
}
