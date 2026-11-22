//! Download history (SQLite), Spank Bank (SQLite), image cache, and on-disk path layout.

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

use crate::api::{Pool, Post};

pub const ORGANIZE_MODES: &[(&str, &str)] = &[
    ("pool", "By pool (or artist if not in a pool)"),
    ("search", "By search / pool (where it came from)"),
    ("artist", "By artist"),
    ("copyright", "By copyright (series / franchise)"),
    ("character", "By character"),
    ("species", "By species"),
    ("rating", "By rating"),
    ("flat", "Flat (everything in one folder)"),
];

pub fn organize_mode_label(mode: &str) -> &str {
    ORGANIZE_MODES
        .iter()
        .find(|(k, _)| *k == mode)
        .map(|(_, l)| *l)
        .unwrap_or(mode)
}

pub fn rating_name(rating: &str) -> &str {
    match rating {
        "s" => "safe",
        "q" => "questionable",
        "e" => "explicit",
        _ => "unknown",
    }
}

fn is_windows_reserved(upper: &str) -> bool {
    matches!(
        upper,
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

pub fn sanitize(name: &str, maxlen: usize) -> String {
    let replaced: String = name
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
                || (c as u32) < 0x20
            {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = replaced.trim_matches([' ', '.']);
    let truncated: String = trimmed.chars().take(maxlen).collect();
    let rstripped = truncated.trim_end_matches([' ', '.']);
    let clean = if rstripped.is_empty() { "_" } else { rstripped };
    if is_windows_reserved(&clean.to_ascii_uppercase()) {
        format!("_{clean}_")
    } else {
        clean.to_string()
    }
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[derive(Debug, Clone)]
pub struct DownloadJob {
    pub post: Post,
    pub base_dir: String,
    pub mode: String,
    pub source: String,
    pub pool_id: Option<u64>,
    pub pool_name: Option<String>,
    pub pool_index: Option<usize>,
    pub write_tags: bool,
    pub skip_downloaded: bool,
}

impl DownloadJob {
    pub fn new(post: Post, base_dir: impl Into<String>, mode: impl Into<String>) -> Self {
        Self {
            post,
            base_dir: base_dir.into(),
            mode: mode.into(),
            source: String::new(),
            pool_id: None,
            pool_name: None,
            pool_index: None,
            write_tags: false,
            skip_downloaded: true,
        }
    }
}

/// Work out where a post should be saved for the chosen organize mode.
pub fn build_target<F>(job: &DownloadJob, mut pool_lookup: F) -> PathBuf
where
    F: FnMut(u64) -> Option<Pool>,
{
    let post = &job.post;
    let base = expand_tilde(&job.base_dir);
    let mode = job.mode.as_str();
    let fname = format!("{}.{}", post.id, post.ext);

    let pool_path = |pid: u64, name: Option<&str>, idx: Option<usize>| -> PathBuf {
        let folder = base
            .join("pools")
            .join(sanitize(&format!("{} ({pid})", name.unwrap_or("pool")), 120));
        match idx {
            Some(i) if i > 0 => folder.join(format!("{i:04}_{fname}")),
            _ => folder.join(&fname),
        }
    };

    if matches!(mode, "search" | "pool") {
        if let Some(pid) = job.pool_id {
            return pool_path(pid, job.pool_name.as_deref(), job.pool_index);
        }
    }

    match mode {
        "pool" => {
            if post.pools.is_empty() {
                let artists = post.artists();
                let artist = artists
                    .first()
                    .map(String::as_str)
                    .unwrap_or("_unknown_artist");
                return base.join("artists").join(sanitize(artist, 120)).join(fname);
            }
            let pid = post.pools[0];
            match pool_lookup(pid) {
                None => pool_path(pid, None, None),
                Some(pool) => {
                    let idx = pool
                        .post_ids
                        .iter()
                        .position(|&id| id == post.id)
                        .map(|i| i + 1);
                    pool_path(pid, Some(&pool.display_name()), idx)
                }
            }
        }
        "search" => {
            let src = if job.source.is_empty() {
                "_no_tags"
            } else {
                job.source.as_str()
            };
            base.join("searches").join(sanitize(src, 120)).join(fname)
        }
        "artist" => {
            let artists = post.artists();
            let artist = artists
                .first()
                .map(String::as_str)
                .unwrap_or("_unknown_artist");
            base.join("artists").join(sanitize(artist, 120)).join(fname)
        }
        "copyright" | "character" | "species" => {
            let sub = match mode {
                "copyright" => "copyrights",
                "character" => "characters",
                "species" => "species",
                _ => unreachable!(),
            };
            let fallback = format!("_unknown_{mode}");
            let val = post
                .tags
                .get(mode)
                .and_then(|v| v.first())
                .map(String::as_str)
                .unwrap_or(&fallback);
            base.join(sub).join(sanitize(val, 120)).join(fname)
        }
        "rating" => base
            .join("ratings")
            .join(rating_name(&post.rating))
            .join(fname),
        _ => base.join(fname),
    }
}

fn unix_timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

struct HistoryInner {
    conn: Connection,
    paths: std::collections::HashMap<u64, String>,
    ids: HashSet<u64>,
}

/// Remembers which posts were downloaded so they can be skipped or marked.
#[derive(Clone)]
pub struct History {
    inner: Arc<Mutex<HistoryInner>>,
}

impl History {
    pub fn open(path: &Path) -> Self {
        let db_target = if let Some(parent) = path.parent() {
            if fs::create_dir_all(parent).is_ok() {
                if !path.exists() {
                    if let (Some(grandparent), Some(fname)) = (parent.parent(), path.file_name()) {
                        let old_path = grandparent.join("e621-grabber").join(fname);
                        if old_path.exists() {
                            let _ = fs::copy(&old_path, path);
                        }
                    }
                }
                Some(path)
            } else {
                None
            }
        } else {
            None
        };

        let conn = db_target
            .and_then(|p| Connection::open(p).ok())
            .unwrap_or_else(|| {
                Connection::open_in_memory().expect("failed to open in-memory history db")
            });

        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS downloads (\
             post_id INTEGER PRIMARY KEY, md5 TEXT, path TEXT, downloaded_at REAL)",
            [],
        );

        let mut paths = std::collections::HashMap::new();
        let mut ids = HashSet::new();
        if let Ok(mut stmt) = conn.prepare("SELECT post_id, path FROM downloads") {
            if let Ok(rows) = stmt.query_map([], |row| {
                let id: i64 = row.get(0)?;
                let p: Option<String> = row.get(1)?;
                Ok((id, p.unwrap_or_default()))
            }) {
                for row in rows.flatten() {
                    if let Ok(pid) = u64::try_from(row.0) {
                        paths.insert(pid, row.1);
                        ids.insert(pid);
                    }
                }
            }
        }

        let hist = Self {
            inner: Arc::new(Mutex::new(HistoryInner { conn, paths, ids })),
        };
        hist.prune_missing();
        hist
    }

    pub fn prune_missing(&self) -> usize {
        let mut g = self.inner.lock().expect("history lock poisoned");
        let missing: Vec<u64> = g
            .paths
            .iter()
            .filter_map(|(&pid, p)| {
                if p.is_empty() || !Path::new(p).exists() {
                    Some(pid)
                } else {
                    None
                }
            })
            .collect();
        if !missing.is_empty() {
            if let Ok(tx) = g.conn.unchecked_transaction() {
                {
                    if let Ok(mut stmt) = tx.prepare("DELETE FROM downloads WHERE post_id = ?") {
                        for pid in &missing {
                            let _ = stmt.execute(params![*pid as i64]);
                        }
                    }
                }
                let _ = tx.commit();
            }
            for pid in &missing {
                g.paths.remove(pid);
                g.ids.remove(pid);
            }
        }
        missing.len()
    }

    pub fn remove(&self, post_id: u64) {
        let mut g = self.inner.lock().expect("history lock poisoned");
        let _ = g
            .conn
            .execute("DELETE FROM downloads WHERE post_id = ?", params![post_id as i64]);
        g.paths.remove(&post_id);
        g.ids.remove(&post_id);
    }

    pub fn has(&self, post_id: u64) -> bool {
        let path_opt = {
            let g = self.inner.lock().expect("history lock poisoned");
            if !g.ids.contains(&post_id) {
                return false;
            }
            g.paths.get(&post_id).cloned()
        };
        match path_opt {
            Some(p) if !p.is_empty() && Path::new(&p).exists() => true,
            _ => {
                self.remove(post_id);
                false
            }
        }
    }

    pub fn add(&self, post: &Post, path: &Path) {
        let p_str = path.to_string_lossy().into_owned();
        let mut g = self.inner.lock().expect("history lock poisoned");
        let _ = g.conn.execute(
            "INSERT OR REPLACE INTO downloads VALUES (?, ?, ?, ?)",
            params![post.id as i64, post.md5, p_str, unix_timestamp()],
        );
        g.paths.insert(post.id, p_str);
        g.ids.insert(post.id);
    }

    pub fn path_for(&self, post_id: u64) -> Option<String> {
        let p = {
            let g = self.inner.lock().expect("history lock poisoned");
            g.paths.get(&post_id).cloned()
        };
        if let Some(ref p_str) = p {
            if !Path::new(p_str).exists() {
                self.remove(post_id);
                return None;
            }
        }
        p
    }

    pub fn clear(&self) {
        let mut g = self.inner.lock().expect("history lock poisoned");
        let _ = g.conn.execute("DELETE FROM downloads", []);
        g.paths.clear();
        g.ids.clear();
    }

    pub fn len(&self) -> usize {
        self.prune_missing();
        self.inner.lock().expect("history lock poisoned").ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn downloaded_ids(&self) -> HashSet<u64> {
        self.prune_missing();
        self.inner.lock().expect("history lock poisoned").ids.clone()
    }
}

struct SpankBankInner {
    conn: Connection,
    ids: HashSet<u64>,
    pool_ids: HashSet<u64>,
    artists: HashSet<String>,
}

/// Persistent bookmark store for online-viewable posts, pools, and artists.
#[derive(Clone)]
pub struct SpankBank {
    inner: Arc<Mutex<SpankBankInner>>,
}

impl SpankBank {
    pub fn open(path: &Path) -> Self {
        let conn = if let Some(parent) = path.parent() {
            if fs::create_dir_all(parent).is_ok() {
                Connection::open(path).unwrap_or_else(|_| {
                    Connection::open_in_memory().expect("failed to open in-memory spank bank")
                })
            } else {
                Connection::open_in_memory().expect("failed to open in-memory spank bank")
            }
        } else {
            Connection::open_in_memory().expect("failed to open in-memory spank bank")
        };

        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS spank_bank (\
             post_id INTEGER PRIMARY KEY, data_json TEXT, added_at REAL)",
            [],
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS spank_bank_pools (\
             pool_id INTEGER PRIMARY KEY, name TEXT, post_count INTEGER, data_json TEXT, added_at REAL)",
            [],
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS spank_bank_artists (\
             name TEXT PRIMARY KEY, added_at REAL)",
            [],
        );

        let mut ids = HashSet::new();
        if let Ok(mut stmt) = conn.prepare("SELECT post_id FROM spank_bank") {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, i64>(0)) {
                for id in rows.flatten() {
                    if let Ok(u) = u64::try_from(id) {
                        ids.insert(u);
                    }
                }
            }
        }

        let mut pool_ids = HashSet::new();
        if let Ok(mut stmt) = conn.prepare("SELECT pool_id FROM spank_bank_pools") {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, i64>(0)) {
                for id in rows.flatten() {
                    if let Ok(u) = u64::try_from(id) {
                        pool_ids.insert(u);
                    }
                }
            }
        }

        let mut artists = HashSet::new();
        if let Ok(mut stmt) = conn.prepare("SELECT name FROM spank_bank_artists") {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for name in rows.flatten() {
                    artists.insert(name);
                }
            }
        }

        Self {
            inner: Arc::new(Mutex::new(SpankBankInner {
                conn,
                ids,
                pool_ids,
                artists,
            })),
        }
    }

    pub fn norm_artist(name: &str) -> String {
        name.trim().to_lowercase().replace(' ', "_")
    }

    pub fn has(&self, post_id: u64) -> bool {
        self.inner
            .lock()
            .expect("spank bank lock poisoned")
            .ids
            .contains(&post_id)
    }

    pub fn has_pool(&self, pool_id: u64) -> bool {
        self.inner
            .lock()
            .expect("spank bank lock poisoned")
            .pool_ids
            .contains(&pool_id)
    }

    pub fn has_artist(&self, name: &str) -> bool {
        let norm = Self::norm_artist(name);
        self.inner
            .lock()
            .expect("spank bank lock poisoned")
            .artists
            .contains(&norm)
    }

    pub fn add_artist(&self, name: &str) {
        let norm = Self::norm_artist(name);
        if norm.is_empty() {
            return;
        }
        let mut g = self.inner.lock().expect("spank bank lock poisoned");
        let _ = g.conn.execute(
            "INSERT OR REPLACE INTO spank_bank_artists VALUES (?, ?)",
            params![norm, unix_timestamp()],
        );
        g.artists.insert(norm);
    }

    pub fn remove_artist(&self, name: &str) {
        let norm = Self::norm_artist(name);
        let mut g = self.inner.lock().expect("spank bank lock poisoned");
        let _ = g
            .conn
            .execute("DELETE FROM spank_bank_artists WHERE name = ?", params![norm]);
        g.artists.remove(&norm);
    }

    pub fn toggle_artist(&self, name: &str) -> bool {
        if self.has_artist(name) {
            self.remove_artist(name);
            false
        } else {
            self.add_artist(name);
            true
        }
    }

    pub fn get_all_artists(&self) -> Vec<String> {
        let g = self.inner.lock().expect("spank bank lock poisoned");
        let mut out = Vec::new();
        if let Ok(mut stmt) = g
            .conn
            .prepare("SELECT name FROM spank_bank_artists ORDER BY added_at DESC")
        {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for name in rows.flatten() {
                    out.push(name);
                }
            }
        }
        out
    }

    pub fn clear_artists(&self) {
        let mut g = self.inner.lock().expect("spank bank lock poisoned");
        let _ = g.conn.execute("DELETE FROM spank_bank_artists", []);
        g.artists.clear();
    }

    pub fn artist_count(&self) -> usize {
        self.inner
            .lock()
            .expect("spank bank lock poisoned")
            .artists
            .len()
    }

    pub fn add(&self, post: &Post) {
        let Ok(data) = serde_json::to_string(post) else {
            return;
        };
        let mut g = self.inner.lock().expect("spank bank lock poisoned");
        let _ = g.conn.execute(
            "INSERT OR REPLACE INTO spank_bank VALUES (?, ?, ?)",
            params![post.id as i64, data, unix_timestamp()],
        );
        g.ids.insert(post.id);
    }

    pub fn add_pool(&self, pool: &Pool) {
        let Ok(data) = serde_json::to_string(pool) else {
            return;
        };
        let mut g = self.inner.lock().expect("spank bank lock poisoned");
        let _ = g.conn.execute(
            "INSERT OR REPLACE INTO spank_bank_pools VALUES (?, ?, ?, ?, ?)",
            params![
                pool.id as i64,
                pool.name,
                pool.post_count() as i64,
                data,
                unix_timestamp()
            ],
        );
        g.pool_ids.insert(pool.id);
    }

    pub fn remove(&self, post_id: u64) {
        let mut g = self.inner.lock().expect("spank bank lock poisoned");
        let _ = g
            .conn
            .execute("DELETE FROM spank_bank WHERE post_id = ?", params![post_id as i64]);
        g.ids.remove(&post_id);
    }

    pub fn remove_pool(&self, pool_id: u64) {
        let mut g = self.inner.lock().expect("spank bank lock poisoned");
        let _ = g.conn.execute(
            "DELETE FROM spank_bank_pools WHERE pool_id = ?",
            params![pool_id as i64],
        );
        g.pool_ids.remove(&pool_id);
    }

    pub fn toggle(&self, post: &Post) -> bool {
        if self.has(post.id) {
            self.remove(post.id);
            false
        } else {
            self.add(post);
            true
        }
    }

    pub fn toggle_pool(&self, pool: &Pool) -> bool {
        if self.has_pool(pool.id) {
            self.remove_pool(pool.id);
            false
        } else {
            self.add_pool(pool);
            true
        }
    }

    pub fn get_all(&self) -> Vec<Post> {
        let g = self.inner.lock().expect("spank bank lock poisoned");
        let mut posts = Vec::new();
        if let Ok(mut stmt) = g
            .conn
            .prepare("SELECT data_json FROM spank_bank ORDER BY added_at DESC")
        {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for raw in rows.flatten() {
                    if let Ok(p) = serde_json::from_str::<Post>(&raw) {
                        posts.push(p);
                    }
                }
            }
        }
        posts
    }

    pub fn get_all_pools(&self) -> Vec<Pool> {
        let g = self.inner.lock().expect("spank bank lock poisoned");
        let mut pools = Vec::new();
        if let Ok(mut stmt) = g
            .conn
            .prepare("SELECT data_json FROM spank_bank_pools ORDER BY added_at DESC")
        {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for raw in rows.flatten() {
                    if let Ok(p) = serde_json::from_str::<Pool>(&raw) {
                        pools.push(p);
                    }
                }
            }
        }
        pools
    }

    pub fn clear(&self) {
        let mut g = self.inner.lock().expect("spank bank lock poisoned");
        let _ = g.conn.execute("DELETE FROM spank_bank", []);
        g.ids.clear();
    }

    pub fn clear_pools(&self) {
        let mut g = self.inner.lock().expect("spank bank lock poisoned");
        let _ = g.conn.execute("DELETE FROM spank_bank_pools", []);
        g.pool_ids.clear();
    }

    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("spank bank lock poisoned")
            .ids
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn pool_count(&self) -> usize {
        self.inner
            .lock()
            .expect("spank bank lock poisoned")
            .pool_ids
            .len()
    }
}

struct ImageCacheInner {
    previews_dir: PathBuf,
    thumbs_dir: PathBuf,
    memory_previews: VecDeque<(u64, Arc<egui::ColorImage>)>,
    memory_thumbs: VecDeque<(u64, Arc<egui::ColorImage>)>,
    max_memory: usize,
}

fn lru_get(
    q: &mut VecDeque<(u64, Arc<egui::ColorImage>)>,
    post_id: u64,
) -> Option<Arc<egui::ColorImage>> {
    let pos = q.iter().position(|(id, _)| *id == post_id)?;
    let entry = q.remove(pos)?;
    let img = Arc::clone(&entry.1);
    q.push_back(entry);
    Some(img)
}

fn lru_set(
    q: &mut VecDeque<(u64, Arc<egui::ColorImage>)>,
    post_id: u64,
    img: Arc<egui::ColorImage>,
    limit: usize,
) {
    if img.width() == 0 || img.height() == 0 {
        return;
    }
    if let Some(pos) = q.iter().position(|(id, _)| *id == post_id) {
        q.remove(pos);
    }
    q.push_back((post_id, img));
    while q.len() > limit {
        q.pop_front();
    }
}

/// Manages local disk cache and bounded LRU in-memory cache for preview images and thumbnails.
#[derive(Clone)]
pub struct ImageCache {
    pub cache_dir: PathBuf,
    inner: Arc<Mutex<ImageCacheInner>>,
}

impl ImageCache {
    pub fn new(cache_dir: &Path, max_memory_items: usize) -> Self {
        let previews_dir = cache_dir.join("previews");
        let thumbs_dir = cache_dir.join("thumbs");
        let _ = fs::create_dir_all(&previews_dir);
        let _ = fs::create_dir_all(&thumbs_dir);
        Self {
            cache_dir: cache_dir.to_path_buf(),
            inner: Arc::new(Mutex::new(ImageCacheInner {
                previews_dir,
                thumbs_dir,
                memory_previews: VecDeque::new(),
                memory_thumbs: VecDeque::new(),
                max_memory: max_memory_items.max(1),
            })),
        }
    }

    pub fn preview_path(&self, post_id: u64, ext: &str) -> PathBuf {
        let clean = ext.trim_start_matches('.').to_ascii_lowercase();
        let clean = if clean.is_empty() { "jpg" } else { &clean };
        let g = self.inner.lock().expect("image cache lock poisoned");
        g.previews_dir.join(format!("{post_id}.{clean}"))
    }

    pub fn thumb_path(&self, post_id: u64, ext: &str) -> PathBuf {
        let clean = ext.trim_start_matches('.').to_ascii_lowercase();
        let clean = if clean.is_empty() { "jpg" } else { &clean };
        let g = self.inner.lock().expect("image cache lock poisoned");
        g.thumbs_dir.join(format!("{post_id}.{clean}"))
    }

    pub fn get_memory_preview(&self, post_id: u64) -> Option<Arc<egui::ColorImage>> {
        let mut g = self.inner.lock().expect("image cache lock poisoned");
        lru_get(&mut g.memory_previews, post_id)
    }

    pub fn set_memory_preview(&self, post_id: u64, img: Arc<egui::ColorImage>) {
        let mut g = self.inner.lock().expect("image cache lock poisoned");
        let limit = g.max_memory;
        lru_set(&mut g.memory_previews, post_id, img, limit);
    }

    pub fn get_memory_thumb(&self, post_id: u64) -> Option<Arc<egui::ColorImage>> {
        let mut g = self.inner.lock().expect("image cache lock poisoned");
        lru_get(&mut g.memory_thumbs, post_id)
    }

    pub fn set_memory_thumb(&self, post_id: u64, img: Arc<egui::ColorImage>) {
        let mut g = self.inner.lock().expect("image cache lock poisoned");
        let limit = g.max_memory * 4;
        lru_set(&mut g.memory_thumbs, post_id, img, limit);
    }

    pub fn find_local_image_path(
        &self,
        post: &Post,
        history: Option<&History>,
        download_dir: Option<&str>,
    ) -> Option<PathBuf> {
        if let Some(hist) = history {
            if let Some(saved) = hist.path_for(post.id) {
                let p = PathBuf::from(saved);
                if p.exists() && fs::metadata(&p).map(|m| m.len() > 0).unwrap_or(false) {
                    return Some(p);
                }
            }
        }
        if let Some(dl_dir) = download_dir {
            if !dl_dir.is_empty() {
                let candidate = expand_tilde(dl_dir).join(format!("{}.{}", post.id, post.ext));
                if candidate.exists()
                    && fs::metadata(&candidate)
                        .map(|m| m.len() > 0)
                        .unwrap_or(false)
                {
                    if let Some(hist) = history {
                        hist.add(post, &candidate);
                    }
                    return Some(candidate);
                }
            }
        }
        let p_cache = self.preview_path(post.id, &post.ext);
        if p_cache.exists()
            && fs::metadata(&p_cache)
                .map(|m| m.len() > 0)
                .unwrap_or(false)
        {
            return Some(p_cache);
        }
        None
    }

    pub fn clear(&self) {
        let mut g = self.inner.lock().expect("image cache lock poisoned");
        g.memory_previews.clear();
        g.memory_thumbs.clear();
    }
}
