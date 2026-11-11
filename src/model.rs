use serde::{Deserialize, Deserializer, Serialize};

pub fn null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Video {
    #[serde(deserialize_with = "null_default")]
    pub id: String,
    #[serde(deserialize_with = "null_default")]
    pub title: String,
    #[serde(deserialize_with = "null_default")]
    pub url: String,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    #[serde(deserialize_with = "null_default")]
    pub extractor: String,
    #[serde(deserialize_with = "null_default")]
    pub channel: String,
    /// Titulo de la lista de origen (para organizar carpetas)
    #[serde(deserialize_with = "null_default")]
    pub playlist: String,
}

#[derive(Clone, Debug)]
pub struct Skipped {
    pub title: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct Extracted {
    pub playlist_title: Option<String>,
    pub items: Vec<Video>,
    pub skipped: Vec<Skipped>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub builtin: bool,
    /// "video" | "audio"
    pub kind: String,
    pub height: Option<u32>,
    /// "native" | "mp4"
    pub container: String,
    /// mp3 | m4a | wav | flac | opus
    pub audio_codec: String,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            id: String::new(),
            name: String::new(),
            builtin: false,
            kind: "video".into(),
            height: None,
            container: "native".into(),
            audio_codec: "mp3".into(),
        }
    }
}

impl Profile {
    pub fn is_audio(&self) -> bool {
        self.kind == "audio"
    }

    pub fn describe(&self) -> String {
        if self.is_audio() {
            match self.audio_codec.as_str() {
                "mp3" => "Solo audio · MP3 192 kbps · máxima compatibilidad".into(),
                "m4a" => "Solo audio · M4A/AAC 192 kbps · ideal para iPhone e iTunes".into(),
                "wav" => "Solo audio · WAV sin pérdida · archivo pesado".into(),
                "flac" => "Solo audio · FLAC sin pérdida · comprimido".into(),
                "opus" => "Solo audio · Opus original · sin recodificar".into(),
                c => format!("Solo audio · {}", c.to_uppercase()),
            }
        } else {
            let h = self.height.map(|h| format!("Hasta {h}p")).unwrap_or_else(|| "Mejor calidad".into());
            let c = if self.container == "mp4" { "MP4 / H.264 · AAC" } else { "formatos nativos" };
            format!("{h} · {c}")
        }
    }
}

pub fn builtin_profiles() -> Vec<Profile> {
    let v = |id: &str, name: &str, height: Option<u32>, container: &str| Profile {
        id: id.into(),
        name: name.into(),
        builtin: true,
        kind: "video".into(),
        height,
        container: container.into(),
        audio_codec: "mp3".into(),
    };
    let a = |id: &str, name: &str, codec: &str| Profile {
        id: id.into(),
        name: name.into(),
        builtin: true,
        kind: "audio".into(),
        height: None,
        container: "native".into(),
        audio_codec: codec.into(),
    };
    vec![
        v("best", "Mejor calidad", None, "native"),
        v("2160p", "4K UHD 2160p", Some(2160), "native"),
        v("1440p", "QHD 1440p", Some(1440), "native"),
        v("1080p", "Full HD 1080p", Some(1080), "native"),
        v("720p", "HD 720p", Some(720), "native"),
        v("480p", "SD 480p", Some(480), "native"),
        v("tv1080", "Smart TV MP4 1080p", Some(1080), "mp4"),
        v("tv720", "Smart TV MP4 720p", Some(720), "mp4"),
        v("tv480", "Smart TV MP4 480p", Some(480), "mp4"),
        a("audio_mp3", "Audio MP3", "mp3"),
        a("audio_m4a", "Audio M4A / AAC", "m4a"),
        a("audio_wav", "Audio WAV", "wav"),
        a("audio_flac", "Audio FLAC", "flac"),
        a("audio_opus", "Audio Opus (original)", "opus"),
    ]
}
