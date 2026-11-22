//! Persistent settings stored as JSON in the XDG / AppData config directory.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MIN_SCORE_ANY: i64 = -1000;

pub const MEDIA_FILTERS: &[(&str, &str)] = &[
    ("All media", ""),
    ("Drawings / Static only", "-animated"),
    ("Animations only", "animated"),
    ("Videos only (WebM/MP4)", "type:webm"),
    ("GIFs only", "type:gif"),
];

pub const QUICK_BLACKLIST_ITEMS: &[(&str, &str)] = &[
    ("gore", "Gore / Violence"),
    ("scat", "Scat / Feces"),
    ("watersports", "Watersports"),
    ("fart", "Fart"),
    ("vore", "Vore"),
    ("feral", "Feral"),
    ("hyper", "Hyper / Macro"),
    ("inflation", "Inflation"),
    ("diaper", "Diaper / Baby"),
    ("young", "Young / Cub"),
    ("humiliation", "Humiliation"),
    ("transformation", "Transformation"),
    ("type:swf", "Flash (.swf)"),
    ("comic", "Comics"),
    ("3d", "3D / CGI"),
    ("photo", "Photos / Real"),
];

pub const COMMON_QUICK_BLACKLIST: &[&str] = &["gore", "scat", "watersports", "young", "type:swf"];

pub const ORDERS: &[(&str, &str)] = &[
    ("Newest", ""),
    ("Highest score", "order:score"),
    ("Most favorited", "order:favcount"),
    ("Random", "order:random"),
    ("Oldest", "order:id"),
];

pub const SLIDESHOW_SPEED_PRESETS: &[(&str, f64)] = &[
    ("0.5s (Fastest)", 0.5),
    ("1s (Very Fast)", 1.0),
    ("2s (Fast)", 2.0),
    ("3s", 3.0),
    ("4s", 4.0),
    ("5s (Normal)", 5.0),
    ("7s", 7.0),
    ("10s (Slow)", 10.0),
    ("15s", 15.0),
    ("20s", 20.0),
    ("30s (Very Slow)", 30.0),
];

pub const DEFAULT_BLACKLIST: &str = "\
# One rule per line, e621 syntax. Examples:
#   gore
#   scat
#   young -rating:s
#   type:swf
";

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn config_base_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(p) = std::env::var_os("APPDATA").filter(|v| !v.is_empty()) {
            return PathBuf::from(p);
        }
        dirs::config_dir().unwrap_or_else(|| home_dir().join("AppData").join("Roaming"))
    }
    #[cfg(not(windows))]
    {
        if let Some(p) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            return PathBuf::from(p);
        }
        home_dir().join(".config")
    }
}

fn data_base_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(p) = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()) {
            return PathBuf::from(p);
        }
        dirs::data_local_dir().unwrap_or_else(|| home_dir().join("AppData").join("Local"))
    }
    #[cfg(not(windows))]
    {
        if let Some(p) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            return PathBuf::from(p);
        }
        home_dir().join(".local").join("share")
    }
}

pub fn config_dir() -> PathBuf {
    config_base_dir().join("e621-cruiser")
}

pub fn data_dir() -> PathBuf {
    data_base_dir().join("e621-cruiser")
}

pub fn settings_file() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn old_config_dir() -> PathBuf {
    config_base_dir().join("e621-grabber")
}

pub fn old_data_dir() -> PathBuf {
    data_base_dir().join("e621-grabber")
}

pub fn default_download_dir() -> String {
    home_dir()
        .join("Pictures")
        .join("e621")
        .to_string_lossy()
        .into_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub username: String,
    pub api_key: String,
    pub blacklist: String,
    pub download_dir: String,
    pub organize_mode: String,
    pub write_tags_file: bool,
    pub skip_downloaded: bool,
    pub concurrent_downloads: usize,
    pub rating_s: bool,
    pub rating_q: bool,
    pub rating_e: bool,
    pub min_score: i64,
    pub order: String,
    pub media_filter: String,
    pub pool_only: bool,
    pub quick_blacklist: Vec<String>,
    pub expand_blacklist: bool,
    pub expand_saved: bool,
    pub saved_searches: Vec<String>,
    pub favorites_user: String,
    pub window_geometry: String,
    pub splitter_state: String,
    pub slideshow_interval: f64,
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    pub left_panel_width: Option<f32>,
    pub right_panel_width: Option<f32>,
    pub first_run_complete: bool,
    #[serde(skip)]
    pub file_path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            username: String::new(),
            api_key: String::new(),
            blacklist: DEFAULT_BLACKLIST.to_string(),
            download_dir: default_download_dir(),
            organize_mode: "pool".to_string(),
            write_tags_file: false,
            skip_downloaded: true,
            concurrent_downloads: 3,
            rating_s: true,
            rating_q: true,
            rating_e: true,
            min_score: MIN_SCORE_ANY,
            order: String::new(),
            media_filter: String::new(),
            pool_only: false,
            quick_blacklist: Vec::new(),
            expand_blacklist: false,
            expand_saved: false,
            saved_searches: Vec::new(),
            favorites_user: String::new(),
            window_geometry: String::new(),
            splitter_state: String::new(),
            slideshow_interval: 5.0,
            window_width: None,
            window_height: None,
            left_panel_width: None,
            right_panel_width: None,
            first_run_complete: false,
            file_path: None,
        }
    }
}

impl Settings {
    /// Load settings from the default config path (migrating from `e621-grabber` if needed).
    pub fn load() -> Self {
        let primary = settings_file();
        let file_to_read = if !primary.exists() {
            let legacy = old_config_dir().join("settings.json");
            if legacy.exists() {
                legacy
            } else {
                primary
            }
        } else {
            primary
        };
        let mut inst = Self::read_json_or_default(&file_to_read);
        inst.file_path = None;
        inst
    }

    /// Load settings from an explicit path (and remember that path for `save()`).
    pub fn load_from(path: &Path) -> Self {
        let mut inst = Self::read_json_or_default(path);
        inst.file_path = Some(path.to_path_buf());
        inst
    }

    fn read_json_or_default(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str::<Self>(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let target = self.file_path.clone().unwrap_or_else(settings_file);
        self.save_to(&target);
    }

    pub fn save_to(&self, target: &Path) {
        let Some(parent) = target.parent() else {
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            return;
        }
        let tmp = if target.extension().is_some() {
            target.with_extension("tmp")
        } else {
            PathBuf::from(format!("{}.tmp", target.display()))
        };
        let Ok(json) = serde_json::to_string_pretty(self) else {
            return;
        };
        if fs::write(&tmp, json).is_err() {
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        let _ = fs::rename(&tmp, target);
    }

    pub fn needs_first_run_setup(&self) -> bool {
        !self.first_run_complete
            && (self.username.trim().is_empty() || self.api_key.trim().is_empty())
    }

    /// Combine custom blacklist text and active quick blacklist items.
    pub fn full_blacklist(&self) -> String {
        let mut parts = Vec::new();
        let trimmed = self.blacklist.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
        if !self.quick_blacklist.is_empty() {
            parts.push("# Quick blacklist selections:".to_string());
            for item in &self.quick_blacklist {
                parts.push(item.clone());
            }
        }
        parts.join("\n")
    }
}

/// Compose the final e621 search query by merging user tags with active UI filters.
pub fn compose_query(base: &str, settings: &Settings) -> String {
    let clean_tokens: Vec<&str> = base.split_whitespace().collect();
    let mut parts: Vec<String> = Vec::new();
    if !clean_tokens.is_empty() {
        parts.push(clean_tokens.join(" "));
    }

    let mut picked: Vec<&str> = Vec::new();
    if settings.rating_s {
        picked.push("s");
    }
    if settings.rating_q {
        picked.push("q");
    }
    if settings.rating_e {
        picked.push("e");
    }
    if picked.len() == 1 {
        parts.push(format!("rating:{}", picked[0]));
    } else if picked.len() == 2 {
        let all: HashSet<&str> = ["s", "q", "e"].into_iter().collect();
        let picked_set: HashSet<&str> = picked.into_iter().collect();
        if let Some(missing) = all.difference(&picked_set).next() {
            parts.push(format!("rating:{missing}").replace("rating:", "-rating:"));
        }
    }

    if settings.min_score > MIN_SCORE_ANY {
        parts.push(format!("score:>={}", settings.min_score));
    }
    if !settings.order.is_empty() && !base.contains("order:") {
        parts.push(settings.order.clone());
    }
    if !settings.media_filter.is_empty() && !base.contains(&settings.media_filter) {
        parts.push(settings.media_filter.clone());
    }
    if settings.pool_only && !base.contains("pool:") {
        parts.push("pool:any".to_string());
    }

    for tag in &settings.quick_blacklist {
        let neg = format!("-{tag}");
        if !base.contains(&neg) && !base.contains(tag.as_str()) {
            parts.push(neg);
        }
    }

    parts.join(" ").trim().to_string()
}
