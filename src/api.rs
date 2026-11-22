//! Minimal, rate-limited client for the e621 JSON API.
//!
//! API docs: https://e621.net/help/api
//! e621 requires a descriptive User-Agent and asks clients to stay under
//! ~2 requests/second. We default to 1 request/second for API calls.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const BASE_URL: &str = "https://e621.net";
pub const APP_NAME: &str = "E621Cruiser";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const POSTS_PER_PAGE: usize = 320;
pub const MAX_PAGE: u32 = 750;

/// Tags that live in the "artist" category on e621 but aren't actual artists.
pub const NON_ARTIST_TAGS: &[&str] = &[
    "conditional_dnp",
    "sound_warning",
    "epilepsy_warning",
    "unknown_artist",
    "anonymous_artist",
    "avoid_posting",
    "third-party_edit",
    "jumpscare_warning",
    "unknown_artist_signature",
    "artist_request",
    "censored_artist",
];

pub fn is_non_artist_tag(tag: &str) -> bool {
    NON_ARTIST_TAGS.contains(&tag)
}

pub fn tag_category_name(category: u8) -> &'static str {
    match category {
        0 => "general",
        1 => "artist",
        3 => "copyright",
        4 => "character",
        5 => "species",
        6 => "invalid",
        7 => "meta",
        8 => "lore",
        _ => "general",
    }
}

pub fn tag_category_id(name: &str) -> u8 {
    match name {
        "general" => 0,
        "artist" => 1,
        "copyright" => 3,
        "character" => 4,
        "species" => 5,
        "invalid" => 6,
        "meta" => 7,
        "lore" => 8,
        _ => 0,
    }
}

pub fn user_agent(username: &str) -> String {
    let trimmed = username.trim();
    let who = if trimmed.is_empty() {
        "anonymous"
    } else {
        trimmed
    };
    format!("{APP_NAME}/{APP_VERSION} (by {who} on e621)")
}

pub fn artist_page_url(name: &str) -> String {
    format!("{BASE_URL}/artists/show_or_new?name={}", urlencoding::encode(name))
}

#[derive(Debug, Clone)]
pub struct ApiError(pub String);

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ApiError {}

/// Thread-safe minimum interval between calls.
pub struct RateLimiter {
    interval: Duration,
    last: Mutex<Option<Instant>>,
}

impl RateLimiter {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: Mutex::new(None),
        }
    }

    pub fn wait(&self) {
        if self.interval.is_zero() {
            return;
        }
        let mut guard = self.last.lock().expect("rate limiter lock poisoned");
        if let Some(prev) = *guard {
            let elapsed = Instant::now().saturating_duration_since(prev);
            if elapsed < self.interval {
                std::thread::sleep(self.interval - elapsed);
            }
        }
        *guard = Some(Instant::now());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub category: u8,
    pub post_count: u64,
}

impl Tag {
    pub fn new(name: impl Into<String>, category: u8, post_count: u64) -> Self {
        Self {
            name: name.into(),
            category,
            post_count,
        }
    }

    pub fn category_name(&self) -> &'static str {
        tag_category_name(self.category)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Post {
    pub id: u64,
    pub md5: String,
    pub ext: String,
    pub file_url: Option<String>,
    pub preview_url: Option<String>,
    pub sample_url: Option<String>,
    pub width: u32,
    pub height: u32,
    pub size: u64,
    pub rating: String,
    pub score: i64,
    pub fav_count: i64,
    pub tags: BTreeMap<String, Vec<String>>,
    pub pools: Vec<u64>,
    pub description: String,
    pub is_favorited: bool,
    pub own_vote: i32,
}

fn opt_str(map: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<String> {
    map.and_then(|m| m.get(key))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn str_or_empty(map: Option<&serde_json::Map<String, Value>>, key: &str) -> String {
    opt_str(map, key).unwrap_or_default()
}

fn bool_val(v: Option<&Value>) -> bool {
    v.and_then(Value::as_bool).unwrap_or(false)
}

fn u32_val(v: Option<&Value>) -> u32 {
    v.and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0)
}

fn u64_val(v: Option<&Value>) -> u64 {
    v.and_then(Value::as_u64).unwrap_or(0)
}

fn i64_val(v: Option<&Value>) -> i64 {
    v.and_then(Value::as_i64).unwrap_or(0)
}

fn i32_val(v: Option<&Value>) -> i32 {
    v.and_then(Value::as_i64)
        .and_then(|n| i32::try_from(n).ok())
        .unwrap_or(0)
}

fn extract_score(v: &Value) -> i64 {
    if let Some(obj) = v.as_object() {
        i64_val(obj.get("total"))
    } else {
        v.as_i64().unwrap_or(0)
    }
}

impl Post {
    /// Parse a post from either e621 v2 (`files`/`stats`) or legacy v1 (`file`/`score`) JSON.
    pub fn from_api_json(v: &Value) -> Result<Self, ApiError> {
        let id = v
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| ApiError("Post JSON missing integer 'id'".to_string()))?;

        let (
            md5,
            ext,
            file_url,
            preview_url,
            sample_url,
            width,
            height,
            size,
            score,
            fav_count,
            is_favorited,
            own_vote,
        ) = if let Some(files) = v.get("files").and_then(Value::as_object) {
            let meta = files.get("meta").and_then(Value::as_object);
            let original = files.get("original").and_then(Value::as_object);
            let preview = files.get("preview").and_then(Value::as_object);
            let sample = files.get("sample").and_then(Value::as_object);
            let stats = v.get("stats").and_then(Value::as_object);
            let has_info = v.get("has").and_then(Value::as_object);

            let file_url = opt_str(original, "url");
            let preview_url = opt_str(preview, "jpg")
                .or_else(|| opt_str(preview, "webp"))
                .or_else(|| opt_str(preview, "url"));
            let has_sample = bool_val(meta.and_then(|m| m.get("has_sample")))
                || bool_val(has_info.and_then(|h| h.get("sample")));
            let sample_candidate = opt_str(sample, "jpg")
                .or_else(|| opt_str(sample, "webp"))
                .or_else(|| opt_str(sample, "url"));
            let sample_url = if has_sample {
                sample_candidate
            } else {
                file_url.clone()
            };

            let md5 = str_or_empty(meta, "md5");
            let ext = str_or_empty(meta, "ext");
            let width = u32_val(original.and_then(|o| o.get("width")));
            let height = u32_val(original.and_then(|o| o.get("height")));
            let size = u64_val(meta.and_then(|m| m.get("size")));

            let score = stats
                .and_then(|s| s.get("score"))
                .map(extract_score)
                .unwrap_or(0);
            let fav_count = i64_val(stats.and_then(|s| s.get("fav_count")));
            let is_favorited = bool_val(stats.and_then(|s| s.get("is_favorited")));
            let own_vote = i32_val(stats.and_then(|s| s.get("vote")));

            (
                md5,
                ext,
                file_url,
                preview_url,
                sample_url,
                width,
                height,
                size,
                score,
                fav_count,
                is_favorited,
                own_vote,
            )
        } else {
            let f = v.get("file").and_then(Value::as_object);
            let preview = v.get("preview").and_then(Value::as_object);
            let sample = v.get("sample").and_then(Value::as_object);
            let score = v.get("score").map(extract_score).unwrap_or(0);

            let file_url = opt_str(f, "url");
            let preview_url = opt_str(preview, "url");
            let has_sample = bool_val(sample.and_then(|s| s.get("has")));
            let sample_url = if has_sample {
                opt_str(sample, "url")
            } else {
                file_url.clone()
            };

            let md5 = str_or_empty(f, "md5");
            let ext = str_or_empty(f, "ext");
            let width = u32_val(f.and_then(|o| o.get("width")));
            let height = u32_val(f.and_then(|o| o.get("height")));
            let size = u64_val(f.and_then(|o| o.get("size")));

            let fav_count = i64_val(v.get("fav_count"));
            let is_favorited = bool_val(v.get("is_favorited"));
            let own_vote = i32_val(v.get("own_vote"));

            (
                md5,
                ext,
                file_url,
                preview_url,
                sample_url,
                width,
                height,
                size,
                score,
                fav_count,
                is_favorited,
                own_vote,
            )
        };

        let mut tags: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if let Some(raw_obj) = v.get("tags").and_then(Value::as_object) {
            for (cat, list_val) in raw_obj {
                let items = list_val
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(ToOwned::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                tags.insert(cat.clone(), items);
            }
        } else if let Some(raw_arr) = v.get("tags").and_then(Value::as_array) {
            let items = raw_arr
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect();
            tags.insert("general".to_string(), items);
        }

        let pools = v
            .get("pools")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default();

        let rating = v
            .get("rating")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let description = v
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        Ok(Self {
            id,
            md5,
            ext,
            file_url,
            preview_url,
            sample_url,
            width,
            height,
            size,
            rating,
            score,
            fav_count,
            tags,
            pools,
            description,
            is_favorited,
            own_vote,
        })
    }

    pub fn all_tags(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        for list in self.tags.values() {
            for t in list {
                out.insert(t.clone());
            }
        }
        out
    }

    pub fn artists(&self) -> Vec<String> {
        self.tags
            .get("artist")
            .map(|list| {
                list.iter()
                    .filter(|a| !is_non_artist_tag(a.as_str()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn is_video(&self) -> bool {
        matches!(self.ext.as_str(), "webm" | "mp4")
    }

    pub fn is_non_image_media(&self) -> bool {
        matches!(self.ext.as_str(), "webm" | "mp4" | "swf")
    }

    pub fn page_url(&self) -> String {
        format!("{BASE_URL}/posts/{}", self.id)
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pool {
    pub id: u64,
    pub name: String,
    pub post_ids: Vec<u64>,
    pub category: String,
    pub description: String,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

impl Default for Pool {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            post_ids: Vec::new(),
            category: String::new(),
            description: String::new(),
            is_active: true,
        }
    }
}

impl Pool {
    pub fn from_api_json(v: &Value) -> Result<Self, ApiError> {
        let id = v
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| ApiError("Pool JSON missing integer 'id'".to_string()))?;

        let name = v
            .get("name")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("pool_{id}"));

        let post_ids = v
            .get("post_ids")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default();

        let category = v
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let description = v
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let is_active = v
            .get("is_active")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        Ok(Self {
            id,
            name,
            post_ids,
            category,
            description,
            is_active,
        })
    }

    pub fn display_name(&self) -> String {
        self.name.replace('_', " ")
    }

    pub fn post_count(&self) -> usize {
        self.post_ids.len()
    }

    pub fn page_url(&self) -> String {
        format!("{BASE_URL}/pools/{}", self.id)
    }
}

pub fn parse_search_posts_response(data: &Value) -> Vec<Post> {
    let raw_posts = if let Some(arr) = data.as_array() {
        arr.as_slice()
    } else if let Some(arr) = data.get("posts").and_then(Value::as_array) {
        arr.as_slice()
    } else {
        &[]
    };
    raw_posts
        .iter()
        .filter_map(|p| Post::from_api_json(p).ok())
        .collect()
}

struct ClientInner {
    username: String,
    api_key: String,
    http: Client,
    limiter: RateLimiter,
    autocomplete_cache: Mutex<HashMap<String, Vec<Tag>>>,
}

#[derive(Clone)]
pub struct E621Client {
    inner: Arc<ClientInner>,
}

impl Default for E621Client {
    fn default() -> Self {
        Self::new("", "", Duration::from_secs(1))
    }
}

fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504 | 520 | 521 | 522)
}

impl E621Client {
    pub fn new(username: &str, api_key: &str, interval: Duration) -> Self {
        let username = username.trim().to_string();
        let api_key = api_key.trim().to_string();

        let mut headers = HeaderMap::new();
        let ua = user_agent(&username);
        if let Ok(hv) = HeaderValue::from_str(&ua) {
            headers.insert(USER_AGENT, hv);
        }
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let http = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            inner: Arc::new(ClientInner {
                username,
                api_key,
                http,
                limiter: RateLimiter::new(interval),
                autocomplete_cache: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn authenticated(&self) -> bool {
        !self.inner.username.is_empty() && !self.inner.api_key.is_empty()
    }

    pub fn username(&self) -> &str {
        &self.inner.username
    }

    pub fn check_connection(&self) -> (bool, String) {
        if self.authenticated() {
            match self.get_json(
                "/users.json",
                &[("search[name_matches]", self.inner.username.as_str())],
                1,
            ) {
                Ok(data) => {
                    if let Some(first) = data.as_array().and_then(|a| a.first()) {
                        let level = first
                            .get("level_string")
                            .and_then(Value::as_str)
                            .unwrap_or("Member");
                        let fav_count = first
                            .get("favorite_count")
                            .and_then(Value::as_u64)
                            .unwrap_or(0);
                        return (
                            true,
                            format!(
                                "Logged in as {} ({}) · {} favorites",
                                self.inner.username, level, fav_count
                            ),
                        );
                    }
                    (
                        true,
                        format!("Connected to e621 as {}", self.inner.username),
                    )
                }
                Err(e) => (false, format!("Connection failed: {e}")),
            }
        } else {
            match self.get_json("/posts.json", &[("limit", "1")], 1) {
                Ok(_) => (true, "Connected to e621 (Anonymous mode)".to_string()),
                Err(e) => (false, format!("Connection failed: {e}")),
            }
        }
    }

    fn apply_auth(&self, req: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        if self.authenticated() {
            req.basic_auth(&self.inner.username, Some(&self.inner.api_key))
        } else {
            req
        }
    }

    fn get_json(
        &self,
        path: &str,
        params: &[(&str, &str)],
        retries: u32,
    ) -> Result<Value, ApiError> {
        let url = format!("{BASE_URL}{path}");
        for attempt in 0..=retries {
            self.inner.limiter.wait();
            let req = self.apply_auth(self.inner.http.get(&url).query(params));
            let resp = match req.send() {
                Ok(r) => r,
                Err(e) => {
                    if attempt == retries {
                        return Err(ApiError(format!("Network error: {e}")));
                    }
                    std::thread::sleep(Duration::from_secs(1u64 << attempt));
                    continue;
                }
            };
            let status = resp.status().as_u16();
            if is_retryable_status(status) && attempt < retries {
                std::thread::sleep(Duration::from_secs(1u64 << (attempt + 1)));
                continue;
            }
            if status == 401 {
                return Err(ApiError(
                    "Authentication failed. Check your username and API key in Settings."
                        .to_string(),
                ));
            }
            if status == 403 {
                return Err(ApiError(
                    "Access denied (403). e621 may be blocking this request.".to_string(),
                ));
            }
            if status == 404 {
                return Err(ApiError("Not found (404).".to_string()));
            }
            if !resp.status().is_success() {
                let txt = resp.text().unwrap_or_default();
                let preview: String = txt.chars().take(200).collect();
                return Err(ApiError(format!("HTTP {status}: {preview}")));
            }
            return resp
                .json::<Value>()
                .map_err(|_| ApiError("Invalid JSON response from e621.".to_string()));
        }
        Err(ApiError("Request failed after retries.".to_string()))
    }

    fn post_form(
        &self,
        path: &str,
        form: &[(&str, &str)],
        retries: u32,
    ) -> Result<Value, ApiError> {
        if !self.authenticated() {
            return Err(ApiError(
                "This action requires logging in with your e621 username and API key in Settings."
                    .to_string(),
            ));
        }
        let url = format!("{BASE_URL}{path}");
        for attempt in 0..=retries {
            self.inner.limiter.wait();
            let req = self.apply_auth(self.inner.http.post(&url).form(form));
            let resp = match req.send() {
                Ok(r) => r,
                Err(e) => {
                    if attempt == retries {
                        return Err(ApiError(format!("Network error: {e}")));
                    }
                    std::thread::sleep(Duration::from_secs(1u64 << attempt));
                    continue;
                }
            };
            let status = resp.status().as_u16();
            if is_retryable_status(status) && attempt < retries {
                std::thread::sleep(Duration::from_secs(1u64 << (attempt + 1)));
                continue;
            }
            if status == 401 {
                return Err(ApiError(
                    "Authentication failed. Check your username and API key in Settings."
                        .to_string(),
                ));
            }
            if status == 403 {
                return Err(ApiError(
                    "Access denied (403). Your account may not have permission for this action."
                        .to_string(),
                ));
            }
            if status == 422 {
                let txt = resp.text().unwrap_or_default();
                let msg = serde_json::from_str::<Value>(&txt)
                    .ok()
                    .and_then(|v| {
                        v.get("message")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                            .or_else(|| v.get("errors").map(|e| e.to_string()))
                    })
                    .unwrap_or_else(|| txt.chars().take(200).collect());
                return Err(ApiError(format!("e621: {msg}")));
            }
            if !resp.status().is_success() {
                let txt = resp.text().unwrap_or_default();
                let preview: String = txt.chars().take(200).collect();
                return Err(ApiError(format!("HTTP {status}: {preview}")));
            }
            return Ok(resp
                .json::<Value>()
                .unwrap_or_else(|_| Value::Object(Default::default())));
        }
        Err(ApiError("Request failed after retries.".to_string()))
    }

    fn delete_json(&self, path: &str, retries: u32) -> Result<Value, ApiError> {
        if !self.authenticated() {
            return Err(ApiError(
                "This action requires logging in with your e621 username and API key in Settings."
                    .to_string(),
            ));
        }
        let url = format!("{BASE_URL}{path}");
        for attempt in 0..=retries {
            self.inner.limiter.wait();
            let req = self.apply_auth(self.inner.http.delete(&url));
            let resp = match req.send() {
                Ok(r) => r,
                Err(e) => {
                    if attempt == retries {
                        return Err(ApiError(format!("Network error: {e}")));
                    }
                    std::thread::sleep(Duration::from_secs(1u64 << attempt));
                    continue;
                }
            };
            let status = resp.status().as_u16();
            if is_retryable_status(status) && attempt < retries {
                std::thread::sleep(Duration::from_secs(1u64 << (attempt + 1)));
                continue;
            }
            if status == 401 {
                return Err(ApiError(
                    "Authentication failed. Check your username and API key in Settings."
                        .to_string(),
                ));
            }
            if status == 403 {
                return Err(ApiError("Access denied (403).".to_string()));
            }
            if status == 404 {
                return Ok(Value::Object(Default::default()));
            }
            if !resp.status().is_success() {
                let txt = resp.text().unwrap_or_default();
                let preview: String = txt.chars().take(200).collect();
                return Err(ApiError(format!("HTTP {status}: {preview}")));
            }
            return Ok(resp
                .json::<Value>()
                .unwrap_or_else(|_| Value::Object(Default::default())));
        }
        Err(ApiError("Request failed after retries.".to_string()))
    }

    // ----------------------------------------------------------- interactions

    pub fn add_favorite(&self, post_id: u64) -> Result<(), ApiError> {
        let id_str = post_id.to_string();
        match self.post_form("/favorites.json", &[("post_id", &id_str)], 2) {
            Ok(_) => Ok(()),
            Err(e) if e.0.to_lowercase().contains("already favorited") => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn remove_favorite(&self, post_id: u64) -> Result<(), ApiError> {
        match self.delete_json(&format!("/favorites/{post_id}.json"), 2) {
            Ok(_) => Ok(()),
            Err(e) => {
                let lower = e.0.to_lowercase();
                if lower.contains("not favorited") || lower.contains("not found") {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn vote_post(&self, post_id: u64, score: i32) -> Result<(), ApiError> {
        let score_str = score.to_string();
        self.post_form(
            &format!("/posts/{post_id}/votes.json"),
            &[("score", &score_str)],
            2,
        )?;
        Ok(())
    }

    pub fn remove_vote(&self, post_id: u64) -> Result<(), ApiError> {
        self.delete_json(&format!("/posts/{post_id}/votes.json"), 2)?;
        Ok(())
    }

    // ------------------------------------------------------------------ posts

    pub fn search_posts(
        &self,
        tags: &str,
        page: u32,
        limit: usize,
    ) -> Result<Vec<Post>, ApiError> {
        let page_s = page.to_string();
        let limit_s = limit.to_string();
        let data = self.get_json(
            "/posts.json",
            &[
                ("tags", tags),
                ("page", &page_s),
                ("limit", &limit_s),
                ("mode", "extended"),
            ],
            3,
        )?;
        Ok(parse_search_posts_response(&data))
    }

    pub fn fetch_all_posts<P, S>(
        &self,
        tags: &str,
        start_page: u32,
        mut progress: P,
        mut should_stop: S,
    ) -> Result<Vec<Post>, ApiError>
    where
        P: FnMut(u32, usize),
        S: FnMut() -> bool,
    {
        let mut out = Vec::new();
        let mut page = start_page;
        while page <= MAX_PAGE {
            if should_stop() {
                break;
            }
            let batch = self.search_posts(tags, page, POSTS_PER_PAGE)?;
            let batch_len = batch.len();
            out.extend(batch);
            progress(page, out.len());
            if batch_len < POSTS_PER_PAGE {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    // ------------------------------------------------------------------- tags

    pub fn autocomplete(&self, prefix: &str) -> Vec<Tag> {
        let prefix = prefix.trim().to_lowercase();
        if prefix.is_empty() {
            return Vec::new();
        }
        if let Some(cached) = self
            .inner
            .autocomplete_cache
            .lock()
            .expect("autocomplete cache lock poisoned")
            .get(&prefix)
            .cloned()
        {
            return cached;
        }

        let mut res: Vec<Tag> = Vec::new();
        if prefix.chars().count() >= 3 {
            if let Ok(data) = self.get_json(
                "/tags/autocomplete.json",
                &[("search[name_matches]", &prefix), ("expiry", "7")],
                1,
            ) {
                if let Some(arr) = data.as_array() {
                    res = parse_tag_list(arr);
                }
            }
        }
        if res.is_empty() {
            let pattern = format!("{prefix}*");
            if let Ok(data) = self.get_json(
                "/tags.json",
                &[
                    ("search[name_matches]", &pattern),
                    ("search[order]", "count"),
                    ("limit", "15"),
                ],
                1,
            ) {
                if let Some(arr) = data.as_array() {
                    res = parse_tag_list(arr);
                }
            }
        }
        if !res.is_empty() {
            self.inner
                .autocomplete_cache
                .lock()
                .expect("autocomplete cache lock poisoned")
                .insert(prefix, res.clone());
        }
        res
    }

    // ------------------------------------------------------------------ pools

    pub fn search_pools(&self, query: &str, page: u32) -> Result<Vec<Pool>, ApiError> {
        let query = query.trim();
        if !query.is_empty() && query.bytes().all(|b| b.is_ascii_digit()) {
            if let Ok(id) = query.parse::<u64>() {
                return Ok(vec![self.get_pool(id)?]);
            }
        }
        let mut q = query.replace(' ', "_");
        if !q.contains('*') {
            q = format!("*{q}*");
        }
        let page_s = page.to_string();
        let data = self.get_json(
            "/pools.json",
            &[
                ("search[name_matches]", &q),
                ("search[order]", "updated_at"),
                ("limit", "75"),
                ("page", &page_s),
            ],
            3,
        )?;
        let Some(arr) = data.as_array() else {
            return Ok(Vec::new());
        };
        Ok(arr
            .iter()
            .filter_map(|p| Pool::from_api_json(p).ok())
            .collect())
    }

    pub fn get_pool(&self, pool_id: u64) -> Result<Pool, ApiError> {
        let data = self.get_json(&format!("/pools/{pool_id}.json"), &[], 3)?;
        Pool::from_api_json(&data)
    }

    pub fn get_pool_posts(&self, pool: &Pool) -> Result<Vec<Post>, ApiError> {
        let mut found: HashMap<u64, Post> = HashMap::new();
        let mut page = 1;
        let query = format!("pool:{}", pool.id);
        while page <= MAX_PAGE {
            let batch = self.search_posts(&query, page, POSTS_PER_PAGE)?;
            let batch_len = batch.len();
            for p in batch {
                found.insert(p.id, p);
            }
            if batch_len < POSTS_PER_PAGE || found.len() >= pool.post_count() {
                break;
            }
            page += 1;
        }
        Ok(pool
            .post_ids
            .iter()
            .filter_map(|id| found.remove(id))
            .collect())
    }
}

fn parse_tag_list(arr: &[Value]) -> Vec<Tag> {
    arr.iter()
        .filter_map(|t| {
            let name = t.get("name")?.as_str()?.to_string();
            let category = t
                .get("category")
                .and_then(Value::as_u64)
                .and_then(|c| u8::try_from(c).ok())
                .unwrap_or(0);
            let post_count = t.get("post_count").and_then(Value::as_u64).unwrap_or(0);
            Some(Tag::new(name, category, post_count))
        })
        .collect()
}
