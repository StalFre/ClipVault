use crate::config::write_json;
use crate::model::{null_default, Profile, Video};
use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAX_RECORDS: usize = 500;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct HistoryRecord {
    #[serde(deserialize_with = "null_default")]
    pub item_id: String,
    #[serde(deserialize_with = "null_default")]
    pub title: String,
    #[serde(deserialize_with = "null_default")]
    pub url: String,
    pub video: Video,
    pub profile: Profile,
    /// done | failed | skipped
    #[serde(deserialize_with = "null_default")]
    pub status: String,
    pub error: Option<String>,
    #[serde(deserialize_with = "null_default")]
    pub dest_dir: String,
    #[serde(deserialize_with = "null_default")]
    pub finished_at: String,
    /// Ruta del archivo final (si se conoce)
    #[serde(deserialize_with = "null_default")]
    pub file: String,
}

fn path() -> PathBuf {
    paths::data_dir().join("history.json")
}

pub fn load() -> Vec<HistoryRecord> {
    let Ok(text) = std::fs::read_to_string(path()) else { return Vec::new() };
    let mut v: Vec<HistoryRecord> = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("history.json ilegible: {e}");
            return Vec::new();
        }
    };
    for r in &mut v {
        // Estados antiguos ("cancelled", "failed (no recuperable)") se normalizan.
        if r.status.starts_with("failed") || r.status == "cancelled" {
            r.status = "failed".into();
        }
    }
    v.sort_by(|a, b| b.finished_at.cmp(&a.finished_at));
    v
}

pub fn save(records: &[HistoryRecord]) {
    write_json(&path(), records);
}
