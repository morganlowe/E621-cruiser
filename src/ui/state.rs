//! Pure, unit-testable UI state models for tabs, viewer, modals, and actions.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rand::seq::SliceRandom;

use crate::api::{Pool, Post};
use crate::settings::Settings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    Search,
    Pool,
    Favorites,
    Artist,
    ArtistFeed,
    SpankBank,
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceKind::Search => "search",
            SourceKind::Pool => "pool",
            SourceKind::Favorites => "favorites",
            SourceKind::Artist => "artist",
            SourceKind::ArtistFeed => "artist_feed",
            SourceKind::SpankBank => "spank_bank",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Source {
    pub kind: SourceKind,
    pub label: String,
    pub query: String,
    pub pool: Option<Pool>,
}

impl Source {
    pub fn new(kind: SourceKind, label: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
            query: query.into(),
            pool: None,
        }
    }

    pub fn with_pool(mut self, pool: Pool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn latest() -> Self {
        Self::new(SourceKind::Search, "latest", "")
    }
}

#[derive(Debug, Clone)]
pub struct TabSnapshot {
    pub source: Source,
    pub posts: Vec<Post>,
    pub page: u32,
    pub exhausted: bool,
    pub hidden: usize,
    pub scroll_offset: f32,
}

#[derive(Debug, Clone)]
pub struct ResultTab {
    pub id: u64,
    pub source: Source,
    pub posts: Vec<Post>,
    pub post_index: HashMap<u64, usize>,
    pub selected_ids: HashSet<u64>,
    pub active_post_id: Option<u64>,
    pub last_clicked_idx: Option<usize>,
    pub downloaded_ids: HashSet<u64>,
    pub page: u32,
    pub gen: u64,
    pub exhausted: bool,
    pub loading: bool,
    pub hidden: usize,
    pub history_back: Vec<TabSnapshot>,
    pub history_forward: Vec<TabSnapshot>,
    pub error_msg: Option<String>,
    pub scroll_offset: f32,
    pub custom_title: Option<String>,
}

impl ResultTab {
    pub fn new(id: u64, source: Source) -> Self {
        Self {
            id,
            source,
            posts: Vec::new(),
            post_index: HashMap::new(),
            selected_ids: HashSet::new(),
            active_post_id: None,
            last_clicked_idx: None,
            downloaded_ids: HashSet::new(),
            page: 1,
            gen: 0,
            exhausted: true,
            loading: false,
            hidden: 0,
            history_back: Vec::new(),
            history_forward: Vec::new(),
            error_msg: None,
            scroll_offset: 0.0,
            custom_title: None,
        }
    }

    pub fn title(&self) -> String {
        if let Some(ref custom) = self.custom_title {
            if self.posts.is_empty() && self.source.query.is_empty() && self.source.pool.is_none() {
                return custom.clone();
            }
        }
        match self.source.kind {
            SourceKind::Pool => {
                if let Some(ref p) = self.source.pool {
                    let short: String = p.display_name().chars().take(20).collect();
                    format!("Pool: {short}")
                } else {
                    "Pool".to_string()
                }
            }
            SourceKind::Favorites => {
                let user = self
                    .source
                    .label
                    .strip_prefix("favorites_")
                    .unwrap_or(&self.source.label);
                let short: String = user.chars().take(20).collect();
                format!("Fav: {short}")
            }
            SourceKind::SpankBank => "Spank Bank".to_string(),
            SourceKind::Artist => {
                let short: String = self.source.label.chars().take(20).collect();
                format!("Artist: {short}")
            }
            SourceKind::ArtistFeed => "Saved Artists".to_string(),
            SourceKind::Search => {
                if self.source.query.is_empty() {
                    "Latest".to_string()
                } else {
                    self.source.query.chars().take(20).collect()
                }
            }
        }
    }

    pub fn tooltip(&self) -> String {
        let detail = if !self.source.label.is_empty() {
            &self.source.label
        } else {
            &self.source.query
        };
        let kind = self.source.kind.as_str();
        let mut chars = kind.chars();
        let cap = match chars.next() {
            Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            None => String::new(),
        };
        format!("{cap}: {detail}")
    }

    pub fn clear_posts(&mut self) {
        self.posts.clear();
        self.post_index.clear();
        self.selected_ids.clear();
        self.active_post_id = None;
        self.last_clicked_idx = None;
        self.downloaded_ids.clear();
        self.error_msg = None;
    }

    pub fn add_posts(&mut self, new_posts: Vec<Post>, downloaded: &HashSet<u64>) -> Vec<Post> {
        let mut added = Vec::new();
        for p in new_posts {
            if self.post_index.contains_key(&p.id) {
                continue;
            }
            if downloaded.contains(&p.id) {
                self.downloaded_ids.insert(p.id);
            }
            let idx = self.posts.len();
            self.post_index.insert(p.id, idx);
            self.posts.push(p.clone());
            added.push(p);
        }
        added
    }

    pub fn snapshot(&self) -> TabSnapshot {
        TabSnapshot {
            source: self.source.clone(),
            posts: self.posts.clone(),
            page: self.page,
            exhausted: self.exhausted,
            hidden: self.hidden,
            scroll_offset: self.scroll_offset,
        }
    }

    pub fn select_all(&mut self) {
        self.selected_ids = self.posts.iter().map(|p| p.id).collect();
    }

    pub fn clear_selection(&mut self) {
        self.selected_ids.clear();
    }

    pub fn selected_posts(&self) -> Vec<Post> {
        self.posts
            .iter()
            .filter(|p| self.selected_ids.contains(&p.id))
            .cloned()
            .collect()
    }

    pub fn active_post(&self) -> Option<&Post> {
        let id = self.active_post_id?;
        let &idx = self.post_index.get(&id)?;
        self.posts.get(idx)
    }

    pub fn update_post_interaction(
        &mut self,
        post_id: u64,
        fav: Option<(bool, i64)>,
        vote: Option<(i32, i64)>,
    ) {
        if let Some(&idx) = self.post_index.get(&post_id) {
            if let Some(p) = self.posts.get_mut(idx) {
                if let Some((is_fav, fav_cnt)) = fav {
                    p.is_favorited = is_fav;
                    p.fav_count = fav_cnt;
                }
                if let Some((own_v, score)) = vote {
                    p.own_vote = own_v;
                    p.score = score;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftTab {
    Search,
    Pools,
    Favorites,
    SpankBank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankSubTab {
    Pools,
    Posts,
    Artists,
}

#[derive(Debug, Clone)]
pub struct ViewerState {
    pub posts: Vec<Post>,
    pub index: usize,
    pub true_fullscreen: bool,
    pub show_tags_panel: bool,
    pub tag_filter: String,
    pub selected_tag: Option<String>,
    pub slideshow_playing: bool,
    pub slideshow_shuffle: bool,
    pub slideshow_loop: bool,
    pub slideshow_interval: f64,
    pub next_tick: Option<Instant>,
    pub shuffle_queue: Vec<usize>,
    pub slideshow_status: String,
    pub loading_error: Option<String>,
}

impl ViewerState {
    pub fn new(
        posts: Vec<Post>,
        index: usize,
        slideshow_interval: f64,
        start_slideshow: bool,
        shuffle: bool,
    ) -> Self {
        let clamped_idx = if posts.is_empty() {
            0
        } else {
            index.min(posts.len() - 1)
        };
        let mut state = Self {
            posts,
            index: clamped_idx,
            true_fullscreen: true,
            show_tags_panel: true,
            tag_filter: String::new(),
            selected_tag: None,
            slideshow_playing: false,
            slideshow_shuffle: shuffle,
            slideshow_loop: true,
            slideshow_interval: slideshow_interval.clamp(0.5, 300.0),
            next_tick: None,
            shuffle_queue: Vec::new(),
            slideshow_status: String::new(),
            loading_error: None,
        };
        if start_slideshow {
            state.start_slideshow();
        }
        state
    }

    pub fn current_post(&self) -> Option<&Post> {
        self.posts.get(self.index)
    }

    pub fn current_post_mut(&mut self) -> Option<&mut Post> {
        self.posts.get_mut(self.index)
    }

    pub fn set_index(&mut self, idx: usize) {
        if !self.posts.is_empty() {
            self.index = idx.min(self.posts.len() - 1);
            self.loading_error = None;
            self.selected_tag = None;
        }
    }

    pub fn prev_post(&mut self) -> bool {
        if self.posts.is_empty() {
            return false;
        }
        if self.index > 0 {
            self.index -= 1;
        } else if self.slideshow_loop && self.posts.len() > 1 {
            self.index = self.posts.len() - 1;
        } else {
            return false;
        }
        self.loading_error = None;
        self.selected_tag = None;
        if self.slideshow_playing {
            self.schedule_next_tick();
        }
        true
    }

    pub fn next_post(&mut self) -> bool {
        if self.posts.is_empty() {
            return false;
        }
        if self.index + 1 < self.posts.len() {
            self.index += 1;
        } else if self.slideshow_loop && self.posts.len() > 1 {
            self.index = 0;
        } else {
            return false;
        }
        self.loading_error = None;
        self.selected_tag = None;
        if self.slideshow_playing {
            self.schedule_next_tick();
        }
        true
    }

    pub fn start_slideshow(&mut self) {
        if self.posts.len() <= 1 {
            self.slideshow_status = "Only 1 post in view".to_string();
            return;
        }
        self.slideshow_playing = true;
        self.update_playing_status();
        self.schedule_next_tick();
    }

    pub fn pause_slideshow(&mut self) {
        self.slideshow_playing = false;
        self.next_tick = None;
        self.slideshow_status = "Paused".to_string();
    }

    pub fn toggle_slideshow(&mut self) {
        if self.slideshow_playing {
            self.pause_slideshow();
        } else {
            self.start_slideshow();
        }
    }

    pub fn set_interval(&mut self, sec: f64) {
        self.slideshow_interval = sec.clamp(0.5, 300.0);
        if self.slideshow_playing {
            self.update_playing_status();
            self.schedule_next_tick();
        }
    }

    pub fn increase_speed(&mut self) -> f64 {
        let cur = self.slideshow_interval;
        let step = if cur <= 3.0 { 0.5 } else { 1.0 };
        let new_val = (cur - step).max(0.5);
        self.set_interval(new_val);
        new_val
    }

    pub fn decrease_speed(&mut self) -> f64 {
        let cur = self.slideshow_interval;
        let step = if cur < 3.0 { 0.5 } else { 1.0 };
        let new_val = (cur + step).min(300.0);
        self.set_interval(new_val);
        new_val
    }

    fn update_playing_status(&mut self) {
        let order = if self.slideshow_shuffle {
            "Shuffle"
        } else {
            "In Order"
        };
        self.slideshow_status = format!("Playing ({:.1}s · {order})", self.slideshow_interval)
            .replace(".0s", "s");
    }

    fn schedule_next_tick(&mut self) {
        self.next_tick =
            Some(Instant::now() + Duration::from_secs_f64(self.slideshow_interval));
    }

    /// Advance one slide according to sequential/shuffle/loop rules.
    /// Returns `true` if the active post index changed.
    pub fn tick_slideshow(&mut self) -> bool {
        if self.posts.is_empty() {
            return false;
        }
        let prev = self.index;
        if self.slideshow_shuffle {
            if self.shuffle_queue.is_empty() {
                let mut idxs: Vec<usize> = (0..self.posts.len()).collect();
                if idxs.len() > 1 {
                    idxs.retain(|&i| i != self.index);
                }
                let mut rng = rand::thread_rng();
                idxs.shuffle(&mut rng);
                self.shuffle_queue = idxs;
            }
            if let Some(next_idx) = self.shuffle_queue.first().copied() {
                self.shuffle_queue.remove(0);
                self.set_index(next_idx);
            }
        } else if self.index + 1 < self.posts.len() {
            self.set_index(self.index + 1);
        } else if self.slideshow_loop {
            self.set_index(0);
        } else {
            self.pause_slideshow();
            self.slideshow_status = "Finished".to_string();
        }
        if self.slideshow_playing {
            self.schedule_next_tick();
        }
        self.index != prev
    }

    pub fn edge_zone_width(total_width: f32) -> f32 {
        (total_width * 0.25).max(120.0)
    }

    /// Navigate previous/next when `x` falls in the left or right edge zone (`0..=total_width`).
    pub fn click_edge_at(&mut self, x: f32, total_width: f32) -> Option<bool> {
        let w = total_width.max(1.0);
        let edge = Self::edge_zone_width(w);
        if x <= edge {
            Some(self.prev_post())
        } else if x >= w - edge {
            Some(self.next_post())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsModalState {
    pub username: String,
    pub api_key: String,
    pub show_api_key: bool,
    pub blacklist: String,
    pub concurrent_downloads: usize,
    pub slideshow_interval: f64,
    pub skip_downloaded: bool,
    pub write_tags_file: bool,
    pub clear_history_requested: bool,
    pub history_count: usize,
    pub test_status: Option<(bool, String)>,
    pub testing: bool,
}

impl SettingsModalState {
    pub fn from_settings(s: &Settings, history_count: usize) -> Self {
        Self {
            username: s.username.clone(),
            api_key: s.api_key.clone(),
            show_api_key: false,
            blacklist: s.blacklist.clone(),
            concurrent_downloads: s.concurrent_downloads.clamp(1, 8),
            slideshow_interval: s.slideshow_interval.clamp(0.5, 300.0),
            skip_downloaded: s.skip_downloaded,
            write_tags_file: s.write_tags_file,
            clear_history_requested: false,
            history_count,
            test_status: None,
            testing: false,
        }
    }

    pub fn apply_to(&self, s: &mut Settings) {
        s.username = self.username.trim().to_string();
        s.api_key = self.api_key.trim().to_string();
        s.blacklist = self.blacklist.clone();
        s.concurrent_downloads = self.concurrent_downloads.clamp(1, 8);
        s.slideshow_interval = self.slideshow_interval.clamp(0.5, 300.0);
        s.skip_downloaded = self.skip_downloaded;
        s.write_tags_file = self.write_tags_file;
    }
}

#[derive(Debug, Clone)]
pub struct FirstRunSplashState {
    pub username: String,
    pub api_key: String,
    pub show_api_key: bool,
    pub testing: bool,
    pub test_status: Option<(bool, String)>,
    pub can_cancel: bool,
}

impl FirstRunSplashState {
    pub fn from_settings(s: &Settings, can_cancel: bool) -> Self {
        Self {
            username: s.username.clone(),
            api_key: s.api_key.clone(),
            show_api_key: false,
            testing: false,
            test_status: None,
            can_cancel,
        }
    }

    pub fn has_credentials(&self) -> bool {
        !self.username.trim().is_empty() && !self.api_key.trim().is_empty()
    }

    pub fn apply_to(&self, s: &mut Settings) {
        s.username = self.username.trim().to_string();
        s.api_key = self.api_key.trim().to_string();
        s.first_run_complete = true;
    }
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    ClearBankPosts,
    ClearBankPools,
    ClearBankArtists,
    DownloadAllResults {
        source: Source,
        posts: Vec<Post>,
    },
    QuitWhileDownloading,
    OpenSettingsForLogin,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum DialogState {
    Info { title: String, message: String },
    Confirm {
        title: String,
        message: String,
        yes_label: String,
        no_label: String,
        action: ConfirmAction,
    },
}

/// Actions emitted by UI widgets and handled sequentially by `E621CruiserApp`.
#[derive(Debug, Clone)]
pub enum UiAction {
    // Tabs & Navigation
    NewTab,
    SwitchTab(usize),
    CloseTab(usize),
    NavigateBack,
    NavigateForward,

    // Search & Filters
    StartSearch {
        raw: Option<String>,
        label: Option<String>,
        kind: SourceKind,
        new_tab: bool,
    },
    SaveFiltersToSettings,
    SelectCommonBlacklist,
    ClearQuickBlacklist,
    SaveSearch,
    RemoveSavedSearch(usize),
    RunSavedSearch(String),
    TriggerAutocomplete(String),

    // Pools & Favorites
    SearchPools,
    OpenPool { pool: Pool, new_tab: bool },
    OpenPoolById { pool_id: u64, new_tab: bool },
    DownloadPool(Pool),
    DownloadPoolById(u64),
    TogglePoolInBank(Pool),
    TogglePoolByIdInBank(u64),
    LoadFavorites,

    // Artists
    OpenArtist { name: String, new_tab: bool },
    ToggleArtistInBank(String),
    AddArtistToBankFromEdit,
    OpenSavedArtistsFeed { new_tab: bool },
    PlayArtistSlideshow { name: String, shuffle: bool },
    RemoveSelectedBankArtists,
    ClearBankArtistsPrompt,

    // Spank Bank
    OpenSpankBank { new_tab: bool },
    PlayBankSlideshow { shuffle: bool },
    PlayBankPoolSlideshow { pool: Pool, shuffle: bool },
    DownloadSelectedBankPosts,
    DownloadAllBankPosts,
    DownloadSelectedBankPools,
    DownloadAllBankPools,
    DownloadEntireBank,
    RemoveSelectedBankPosts,
    ClearBankPostsPrompt,
    RemoveSelectedBankPools,
    ClearBankPoolsPrompt,
    TogglePostInBank(Post),
    ToggleCurrentSourceInBank,

    // Tags
    ToggleTagInSearch(String),
    ExcludeTagInSearch(String),
    SearchOnlyTag(String),
    CopyText(String),

    // Post Interactions & Viewer
    ToggleFavorite(Post),
    ToggleUpvote(Post),
    OpenViewer {
        post: Post,
        posts: Option<Vec<Post>>,
        start_slideshow: bool,
        shuffle: bool,
    },
    CloseViewer,
    ViewerPostChanged(Post),
    ViewerIntervalChanged(f64),
    StartTabSlideshow,

    // Grid & Downloads
    LoadNextPage,
    GridPostSelected {
        post: Post,
        ctrl: bool,
        shift: bool,
        index: usize,
    },
    SelectAllPosts,
    SelectNoPosts,
    DownloadPosts(Vec<Post>),
    DownloadSelectedPosts,
    DownloadAllResults,
    DownloadCurrentPoolOrBank,
    CancelDownloads,
    BrowseDownloadDir,
    OpenDownloadDir,
    OrganizeModeChanged,
    SaveTagsFileToggled,

    // Menus & Windows
    OpenSettings,
    OpenFirstRunSplash,
    TestConnectionManual,
    TestConnectionInModal,
    TestConnectionInSplash,
    SaveSettingsModal,
    CancelSettingsModal,
    SaveFirstRunSplash,
    SkipFirstRunSplash,
    CancelFirstRunSplash,
    OpenUrl(String),
    OpenPath(PathBuf),
    RequestQuit,
}
