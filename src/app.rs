//! Main eframe application orchestrating tabs, workers, downloads, and views.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use egui::{ColorImage, Context, Key, Modifiers, TextureHandle, TextureOptions, ViewportCommand};
use rand::seq::SliceRandom;

use crate::api::{E621Client, Pool, Post, POSTS_PER_PAGE};
use crate::blacklist::Blacklist;
use crate::downloader::{make_download_jobs, DownloadManager};
use crate::settings::{compose_query, data_dir, Settings, COMMON_QUICK_BLACKLIST};
use crate::storage::{
    expand_tilde, organize_mode_label, History, ImageCache, SpankBank,
};
use crate::tags::TagAutocompleteState;
use crate::ui::download_panel::show_download_panel;
use crate::ui::first_run_splash::show_first_run_splash;
use crate::ui::grid::{show_results_tab_bar, show_tab_contents};
use crate::ui::inspector::show_inspector;
use crate::ui::left_panel::show_left_panel;
use crate::ui::settings_modal::show_settings_modal;
use crate::ui::state::{
    BankSubTab, ConfirmAction, DialogState, FirstRunSplashState, LeftTab, ResultTab,
    SettingsModalState, Source, SourceKind, UiAction, ViewerState,
};
use crate::ui::theme;
use crate::ui::viewer::show_viewer;
use crate::workers::{ApiTask, BgEvent, DownloadStatus, PoolMetaAction, WorkerPool};

const MAX_FEED_ARTISTS: usize = 25;
const MAX_THUMB_TEXTURES: usize = 400;
const MAX_PREVIEW_TEXTURES: usize = 40;

pub struct E621CruiserApp {
    settings: Settings,
    history: History,
    spank_bank: SpankBank,
    image_cache: ImageCache,
    client: E621Client,
    blacklist: Blacklist,
    workers: WorkerPool,
    downloads: DownloadManager,
    ev_rx: mpsc::Receiver<BgEvent>,

    tabs: Vec<ResultTab>,
    active_tab: usize,
    next_tab_id: u64,
    pending_scroll_restore: Option<f32>,

    left_tab: LeftTab,
    bank_subtab: BankSubTab,
    show_inspector: bool,

    tag_query: String,
    autocomplete: TagAutocompleteState,
    selected_saved_idx: Option<usize>,
    focus_search_requested: bool,

    pool_query: String,
    pool_results: Vec<Pool>,
    selected_pool_idx: Option<usize>,

    fav_user: String,

    bank_posts: Vec<Post>,
    bank_pools: Vec<Pool>,
    bank_artists: Vec<String>,
    bank_pools_filter: String,
    selected_bank_pool_ids: HashSet<u64>,
    last_bank_pool_id: Option<u64>,
    bank_posts_filter: String,
    selected_bank_post_ids: HashSet<u64>,
    bank_artists_filter: String,
    bank_artist_add: String,
    selected_bank_artists: HashSet<String>,
    last_bank_artist: Option<String>,

    inspector_filter: String,

    thumb_textures: HashMap<u64, TextureHandle>,
    thumb_lru: VecDeque<u64>,
    preview_textures: HashMap<u64, TextureHandle>,
    preview_lru: VecDeque<u64>,

    dl_done: usize,
    dl_total: usize,
    logs: VecDeque<String>,

    status_msg: String,
    status_expires: Option<Instant>,
    auth_status: String,

    viewer: Option<ViewerState>,
    settings_modal: Option<SettingsModalState>,
    first_run_splash: Option<FirstRunSplashState>,
    dialog: Option<DialogState>,
    app_logo: Option<TextureHandle>,

    stop_fetch: Arc<AtomicBool>,
    allow_close: bool,
    initial_connect_done: bool,
    style_initialized: bool,
}

fn load_logo_texture(ctx: &Context) -> Option<TextureHandle> {
    let bytes = include_bytes!("../assets/e621-cruiser.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let cimg = ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture("e621_cruiser_logo", cimg, TextureOptions::LINEAR))
}

impl E621CruiserApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::configure_style(&cc.egui_ctx);
        let app_logo = load_logo_texture(&cc.egui_ctx);

        let settings = Settings::load();
        let ddir = data_dir();
        let history = History::open(&ddir.join("history.db"));
        let spank_bank = SpankBank::open(&ddir.join("spank_bank.db"));
        let image_cache = ImageCache::new(&ddir.join("cache"), 120);

        let (ev_tx, ev_rx) = mpsc::channel();
        let workers = WorkerPool::new(ev_tx.clone());
        workers.set_egui_ctx(cc.egui_ctx.clone());

        let downloads = DownloadManager::new(
            history.clone(),
            Some(image_cache.clone()),
            ev_tx,
            workers.ctx_handle(),
        );

        let client = E621Client::new(
            &settings.username,
            &settings.api_key,
            Duration::from_secs(1),
        );
        downloads.configure(client.clone(), settings.concurrent_downloads);
        let blacklist = Blacklist::new(&settings.full_blacklist());

        let auth_status = if client.authenticated() {
            format!("Logged in as {}", settings.username)
        } else {
            "Anonymous (add API key in Settings)".to_string()
        };

        let mut initial_tab = ResultTab::new(1, Source::latest());
        initial_tab.custom_title = Some("Search".to_string());

        let bank_posts = spank_bank.get_all();
        let bank_pools = spank_bank.get_all_pools();
        let bank_artists = spank_bank.get_all_artists();
        let fav_user = settings.favorites_user.clone();
        let first_run_splash = if settings.needs_first_run_setup() {
            Some(FirstRunSplashState::from_settings(&settings, false))
        } else {
            None
        };

        Self {
            settings,
            history,
            spank_bank,
            image_cache,
            client,
            blacklist,
            workers,
            downloads,
            ev_rx,

            tabs: vec![initial_tab],
            active_tab: 0,
            next_tab_id: 2,
            pending_scroll_restore: None,

            left_tab: LeftTab::Search,
            bank_subtab: BankSubTab::Pools,
            show_inspector: true,

            tag_query: String::new(),
            autocomplete: TagAutocompleteState::new(),
            selected_saved_idx: None,
            focus_search_requested: false,

            pool_query: String::new(),
            pool_results: Vec::new(),
            selected_pool_idx: None,

            fav_user,

            bank_posts,
            bank_pools,
            bank_artists,
            bank_pools_filter: String::new(),
            selected_bank_pool_ids: HashSet::new(),
            last_bank_pool_id: None,
            bank_posts_filter: String::new(),
            selected_bank_post_ids: HashSet::new(),
            bank_artists_filter: String::new(),
            bank_artist_add: String::new(),
            selected_bank_artists: HashSet::new(),
            last_bank_artist: None,

            inspector_filter: String::new(),

            thumb_textures: HashMap::new(),
            thumb_lru: VecDeque::new(),
            preview_textures: HashMap::new(),
            preview_lru: VecDeque::new(),

            dl_done: 0,
            dl_total: 0,
            logs: VecDeque::new(),

            status_msg: "Ready".to_string(),
            status_expires: None,
            auth_status,

            viewer: None,
            settings_modal: None,
            first_run_splash,
            dialog: None,
            app_logo,

            stop_fetch: Arc::new(AtomicBool::new(false)),
            allow_close: false,
            initial_connect_done: false,
            style_initialized: true,
        }
    }

    fn current_tab(&self) -> &ResultTab {
        &self.tabs[self.active_tab.min(self.tabs.len() - 1)]
    }

    fn current_tab_mut(&mut self) -> &mut ResultTab {
        let idx = self.active_tab.min(self.tabs.len() - 1);
        &mut self.tabs[idx]
    }

    fn set_status(&mut self, msg: impl Into<String>, timeout_ms: Option<u64>) {
        self.status_msg = msg.into();
        self.status_expires =
            timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
    }

    fn log(&mut self, msg: impl Into<String>) {
        self.logs.push_back(msg.into());
        while self.logs.len() > 1000 {
            self.logs.pop_front();
        }
    }

    fn refresh_bank_cache(&mut self) {
        self.bank_posts = self.spank_bank.get_all();
        self.bank_pools = self.spank_bank.get_all_pools();
        self.bank_artists = self.spank_bank.get_all_artists();
    }

    fn apply_settings(&mut self) {
        self.client = E621Client::new(
            &self.settings.username,
            &self.settings.api_key,
            Duration::from_secs(1),
        );
        self.blacklist = Blacklist::new(&self.settings.full_blacklist());
        self.downloads
            .configure(self.client.clone(), self.settings.concurrent_downloads);
        self.auth_status = if self.client.authenticated() {
            format!("Logged in as {}", self.settings.username)
        } else {
            "Anonymous (add API key in Settings)".to_string()
        };
    }

    fn store_thumb_texture(&mut self, ctx: &Context, post_id: u64, img: &ColorImage) {
        if img.width() == 0 || img.height() == 0 {
            return;
        }
        let tex = ctx.load_texture(
            format!("thumb_{post_id}"),
            img.clone(),
            TextureOptions::LINEAR,
        );
        self.thumb_textures.insert(post_id, tex);
        self.thumb_lru.retain(|&id| id != post_id);
        self.thumb_lru.push_back(post_id);
        while self.thumb_lru.len() > MAX_THUMB_TEXTURES {
            if let Some(old) = self.thumb_lru.pop_front() {
                self.thumb_textures.remove(&old);
            }
        }
    }

    fn store_preview_texture(&mut self, ctx: &Context, post_id: u64, img: &ColorImage) {
        if img.width() == 0 || img.height() == 0 {
            return;
        }
        let tex = ctx.load_texture(
            format!("preview_{post_id}"),
            img.clone(),
            TextureOptions::LINEAR,
        );
        self.preview_textures.insert(post_id, tex);
        self.preview_lru.retain(|&id| id != post_id);
        self.preview_lru.push_back(post_id);
        while self.preview_lru.len() > MAX_PREVIEW_TEXTURES {
            if let Some(old) = self.preview_lru.pop_front() {
                self.preview_textures.remove(&old);
            }
        }
    }

    fn drain_events(&mut self, ctx: &Context) {
        let mut events = Vec::new();
        while let Ok(ev) = self.ev_rx.try_recv() {
            events.push(ev);
        }
        for ev in events {
            self.handle_event(ctx, ev);
        }
    }

    fn handle_event(&mut self, ctx: &Context, ev: BgEvent) {
        match ev {
            BgEvent::ConnectionChecked { ok, msg, manual } => {
                if manual {
                    if ok {
                        self.set_status(format!("[OK] {msg}"), Some(6000));
                        self.dialog = Some(DialogState::Info {
                            title: "Connection Successful".to_string(),
                            message: msg,
                        });
                    } else {
                        self.set_status(format!("[Failed] {msg}"), Some(8000));
                        self.dialog = Some(DialogState::Info {
                            title: "Connection Failed".to_string(),
                            message: msg,
                        });
                    }
                } else if ok {
                    self.set_status(format!("[OK] {msg}"), Some(5000));
                    self.auth_status = if !self.client.authenticated() {
                        "Anonymous".to_string()
                    } else {
                        self.settings.username.clone()
                    };
                } else {
                    self.set_status(format!("[Failed] {msg}"), Some(8000));
                    self.auth_status = "Connection error".to_string();
                }
            }
            BgEvent::SettingsTestResult { ok, msg } => {
                if let Some(ref mut modal) = self.settings_modal {
                    modal.testing = false;
                    modal.test_status = Some((ok, msg.clone()));
                }
                if let Some(ref mut splash) = self.first_run_splash {
                    splash.testing = false;
                    splash.test_status = Some((ok, msg));
                }
            }
            BgEvent::AutocompleteResult { token: _, tags } => {
                let cursor = self
                    .autocomplete
                    .last_cursor_char
                    .unwrap_or_else(|| self.tag_query.chars().count());
                self.autocomplete
                    .set_suggestions(&tags, &self.tag_query, cursor);
            }
            BgEvent::PostsPageLoaded { tab_id, gen, posts } => {
                let Some(tab_idx) = self.tabs.iter().position(|t| t.id == tab_id) else {
                    return;
                };
                if self.tabs[tab_idx].gen != gen {
                    return;
                }
                self.tabs[tab_idx].loading = false;
                if posts.len() < POSTS_PER_PAGE || self.tabs[tab_idx].page >= 750 {
                    self.tabs[tab_idx].exhausted = true;
                }
                self.tabs[tab_idx].page += 1;
                self.show_posts_in_tab(ctx, tab_idx, &posts);
                self.set_status("Ready", Some(3000));
            }
            BgEvent::PoolPostsLoaded { tab_id, gen, posts } => {
                let Some(tab_idx) = self.tabs.iter().position(|t| t.id == tab_id) else {
                    return;
                };
                if self.tabs[tab_idx].gen != gen {
                    return;
                }
                self.tabs[tab_idx].loading = false;
                self.show_posts_in_tab(ctx, tab_idx, &posts);
                self.set_status("Pool loaded", Some(3000));
            }
            BgEvent::PoolMetadataLoaded { pool, action } => match action {
                PoolMetaAction::Open => self.open_pool(ctx, pool, false),
                PoolMetaAction::OpenNewTab => self.open_pool(ctx, pool, true),
                PoolMetaAction::ToggleBank => {
                    self.toggle_pool_in_bank(&pool);
                }
                PoolMetaAction::Download => {
                    let src = Source::new(
                        SourceKind::Pool,
                        pool.display_name(),
                        format!("pool:{}", pool.id),
                    )
                    .with_pool(pool.clone());
                    self.set_status(
                        format!("Fetching pool {} for download…", pool.display_name()),
                        None,
                    );
                    let client = self.client.clone();
                    let bl = self.blacklist.clone();
                    let dl = self.downloads.clone();
                    let base = self.effective_download_dir();
                    let mode = self.settings.organize_mode.clone();
                    let write_tags = self.settings.write_tags_file;
                    let skip = self.settings.skip_downloaded;
                    let saved_pools = self.bank_pools.clone();
                    let ev_tx = self.workers.event_sender();
                    let ctx_h = self.workers.ctx_handle();
                    std::thread::spawn(move || match client.get_pool_posts(&pool) {
                        Ok(posts) => {
                            let (kept, _) = bl.filter(&posts);
                            let jobs = make_download_jobs(
                                &kept,
                                &src,
                                &base,
                                &mode,
                                write_tags,
                                skip,
                                &saved_pools,
                            );
                            dl.enqueue(jobs);
                        }
                        Err(e) => {
                            crate::workers::send_event(
                                &ev_tx,
                                &ctx_h,
                                BgEvent::ApiError {
                                    tab_id: None,
                                    gen: None,
                                    err: e.0,
                                },
                            );
                        }
                    });
                }
            },
            BgEvent::PoolsSearchResult { pools } => {
                let count = pools.len();
                self.pool_results = pools;
                self.selected_pool_idx = None;
                self.set_status(format!("{count} pool(s) found"), Some(4000));
            }
            BgEvent::SlideshowPostsLoaded {
                posts,
                shuffle,
                title,
            } => {
                let (kept, _) = self.blacklist.filter(&posts);
                if kept.is_empty() {
                    self.dialog = Some(DialogState::Info {
                        title: "No posts".to_string(),
                        message: format!("No posts available to display for {title}."),
                    });
                    return;
                }
                let mut ordered = kept;
                if shuffle {
                    let mut rng = rand::thread_rng();
                    ordered.shuffle(&mut rng);
                }
                if let Some(first) = ordered.first().cloned() {
                    self.open_viewer(ctx, first, Some(ordered), true, shuffle);
                }
            }
            BgEvent::DownloadAllCollected {
                tab_id,
                gen,
                source,
                shown,
                more,
            } => {
                if let Some(tab) = self.tabs.iter().find(|t| t.id == tab_id) {
                    if tab.gen != gen {
                        return;
                    }
                }
                let (kept, hidden) = self.blacklist.filter(&more);
                let seen: HashSet<u64> = shown.iter().map(|p| p.id).collect();
                let mut everything = shown;
                for p in kept {
                    if !seen.contains(&p.id) {
                        everything.push(p);
                    }
                }
                let new_cnt = everything
                    .iter()
                    .filter(|p| !self.history.has(p.id))
                    .count();
                let q_label = if source.query.is_empty() {
                    "latest"
                } else {
                    source.query.as_str()
                };
                let hidden_txt = if hidden > 0 {
                    format!(", {hidden} more hidden by blacklist")
                } else {
                    String::new()
                };
                self.set_status("Ready", Some(3000));
                self.dialog = Some(DialogState::Confirm {
                    title: "Download all results".to_string(),
                    message: format!(
                        "Found {} posts for “{q_label}” ({new_cnt} not downloaded yet{hidden_txt}).\n\nDownload them all?",
                        everything.len()
                    ),
                    yes_label: "Yes".to_string(),
                    no_label: "No".to_string(),
                    action: ConfirmAction::DownloadAllResults {
                        source,
                        posts: everything,
                    },
                });
            }
            BgEvent::FetchAllStatus { msg } => {
                self.set_status(msg, None);
            }
            BgEvent::ApiError { tab_id, gen, err } => {
                if let (Some(tid), Some(g)) = (tab_id, gen) {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tid) {
                        if tab.gen != g {
                            return;
                        }
                        tab.loading = false;
                        tab.exhausted = true;
                        tab.error_msg = Some(err.clone());
                    }
                }
                self.set_status(format!("Error: {err}"), Some(8000));
                self.log(format!("Error: {err}"));
            }
            BgEvent::FavoriteToggled {
                post_id,
                is_favorited,
            } => {
                let msg = format!(
                    "{} post #{post_id} on e621{}",
                    if is_favorited {
                        "Favorited"
                    } else {
                        "Removed from favorites:"
                    },
                    if is_favorited {
                        " (artist credited)"
                    } else {
                        ""
                    }
                );
                self.apply_interaction_update(post_id, Some(is_favorited), None);
                self.set_status(&msg, Some(4000));
                self.log(msg);
            }
            BgEvent::FavoriteError(err) => {
                self.set_status(format!("Failed to update favorite: {err}"), Some(5000));
                self.dialog = Some(DialogState::Info {
                    title: "Favorite Failed".to_string(),
                    message: format!("Could not update favorite on e621:\n{err}"),
                });
            }
            BgEvent::UpvoteToggled { post_id, own_vote } => {
                let msg = if own_vote == 1 {
                    format!("Upvoted post #{post_id} on e621 (+1)")
                } else {
                    format!("Removed upvote from post #{post_id} on e621")
                };
                self.apply_interaction_update(post_id, None, Some(own_vote));
                self.set_status(&msg, Some(4000));
                self.log(msg);
            }
            BgEvent::UpvoteError(err) => {
                self.set_status(format!("Failed to vote: {err}"), Some(5000));
                self.dialog = Some(DialogState::Info {
                    title: "Vote Failed".to_string(),
                    message: format!("Could not vote on e621:\n{err}"),
                });
            }
            BgEvent::ThumbLoaded {
                tab_id,
                gen,
                post_id,
                image,
            } => {
                let matches = self
                    .tabs
                    .iter()
                    .any(|t| t.id == tab_id && t.gen == gen);
                if matches {
                    self.store_thumb_texture(ctx, post_id, &image);
                }
            }
            BgEvent::PreviewLoaded {
                post_id,
                image,
                for_viewer: _,
            } => {
                self.store_preview_texture(ctx, post_id, &image);
                if let Some(ref mut v) = self.viewer {
                    if v.current_post().map(|p| p.id) == Some(post_id) {
                        v.loading_error = None;
                    }
                }
                self.prefetch_next_slide(ctx);
            }
            BgEvent::PreviewError { post_id, err } => {
                if let Some(ref mut v) = self.viewer {
                    if v.current_post().map(|p| p.id) == Some(post_id) {
                        v.loading_error = Some(err);
                    }
                }
            }
            BgEvent::DownloadProgress { done, total } => {
                self.dl_done = done;
                self.dl_total = total;
            }
            BgEvent::DownloadItemFinished {
                post_id,
                status,
                detail,
            } => {
                if matches!(status, DownloadStatus::Ok | DownloadStatus::Skipped) {
                    for tab in &mut self.tabs {
                        if tab.post_index.contains_key(&post_id) {
                            tab.downloaded_ids.insert(post_id);
                        }
                    }
                }
                match status {
                    DownloadStatus::Ok => self.log(format!("[OK] #{post_id} -> {detail}")),
                    DownloadStatus::Error => self.log(format!("[Error] #{post_id}: {detail}")),
                    DownloadStatus::Skipped => {
                        self.log(format!("[Skipped] #{post_id} ({detail})"))
                    }
                    DownloadStatus::Cancelled => {}
                }
            }
            BgEvent::DownloadsIdle => {
                self.log("Downloads finished.");
            }
            BgEvent::FolderPicked(Some(path)) => {
                self.settings.download_dir = path.to_string_lossy().into_owned();
                self.settings.save();
            }
            BgEvent::FolderPicked(None) => {}
        }
    }

    fn apply_interaction_update(
        &mut self,
        post_id: u64,
        new_fav: Option<bool>,
        new_vote: Option<i32>,
    ) {
        let update_post = |p: &mut Post| {
            if let Some(fav) = new_fav {
                if p.is_favorited != fav {
                    p.is_favorited = fav;
                    if fav {
                        p.fav_count += 1;
                    } else {
                        p.fav_count = (p.fav_count - 1).max(0);
                    }
                }
            }
            if let Some(vote) = new_vote {
                if p.own_vote != vote {
                    let diff = vote - p.own_vote;
                    p.score += diff as i64;
                    p.own_vote = vote;
                }
            }
        };

        for tab in &mut self.tabs {
            if let Some(&idx) = tab.post_index.get(&post_id) {
                if let Some(p) = tab.posts.get_mut(idx) {
                    update_post(p);
                }
            }
        }
        if let Some(ref mut v) = self.viewer {
            for p in &mut v.posts {
                if p.id == post_id {
                    update_post(p);
                }
            }
        }
        for p in &mut self.bank_posts {
            if p.id == post_id {
                update_post(p);
                if self.spank_bank.has(p.id) {
                    self.spank_bank.add(p);
                }
            }
        }
    }

    fn show_posts_in_tab(&mut self, ctx: &Context, tab_idx: usize, posts: &[Post]) {
        let (kept, hidden) = self.blacklist.filter(posts);
        for p in &kept {
            self.autocomplete.learn_post_tags(p);
        }
        let downloaded_set: HashSet<u64> = kept
            .iter()
            .filter(|p| self.history.has(p.id))
            .map(|p| p.id)
            .collect();

        let (tab_id, tab_gen, added) = {
            let tab = &mut self.tabs[tab_idx];
            tab.hidden += hidden;
            let added = tab.add_posts(kept, &downloaded_set);
            (tab.id, tab.gen, added)
        };

        for p in added {
            if self.thumb_textures.contains_key(&p.id) {
                continue;
            }
            if let Some(mem) = self.image_cache.get_memory_thumb(p.id) {
                self.store_thumb_texture(ctx, p.id, &mem);
            } else if let Some(ref url) = p.preview_url {
                let cache_path = self.image_cache.thumb_path(p.id, &p.ext);
                self.workers.submit_thumb(
                    tab_id,
                    tab_gen,
                    p.id,
                    url.clone(),
                    cache_path,
                    self.image_cache.clone(),
                );
            }
        }
    }

    fn effective_download_dir(&self) -> String {
        let d = self.settings.download_dir.trim();
        if d.is_empty() {
            crate::settings::default_download_dir()
        } else {
            d.to_string()
        }
    }

    fn source_in_bank(&self, s: &Source) -> bool {
        match s.kind {
            SourceKind::Pool => s
                .pool
                .as_ref()
                .map(|p| self.spank_bank.has_pool(p.id))
                .unwrap_or(false),
            SourceKind::Artist => self.spank_bank.has_artist(&s.label),
            _ => false,
        }
    }

    fn begin(&mut self, source: Source, new_tab: bool) {
        if new_tab {
            let id = self.next_tab_id;
            self.next_tab_id += 1;
            let mut t = ResultTab::new(id, source.clone());
            t.custom_title = Some("Loading…".to_string());
            self.tabs.push(t);
            self.active_tab = self.tabs.len() - 1;
        } else {
            let idx = self.active_tab.min(self.tabs.len() - 1);
            let should_snap = {
                let tab = &self.tabs[idx];
                !tab.posts.is_empty() || !tab.source.query.is_empty()
            };
            if should_snap {
                let snap = self.tabs[idx].snapshot();
                self.tabs[idx].history_back.push(snap);
                self.tabs[idx].history_forward.clear();
            }
        }

        self.stop_fetch.store(true, Ordering::Relaxed);
        self.stop_fetch = Arc::new(AtomicBool::new(false));
        self.workers.clear_pending_thumbs();

        let tab = self.current_tab_mut();
        tab.gen += 1;
        tab.clear_posts();
        tab.source = source;
        tab.page = 1;
        tab.hidden = 0;
        tab.loading = false;
        tab.exhausted = true;
        tab.scroll_offset = 0.0;
        self.pending_scroll_restore = Some(0.0);
    }

    fn start_search(
        &mut self,
        raw: Option<String>,
        label: Option<String>,
        kind: SourceKind,
        new_tab: bool,
    ) {
        if !self.settings.rating_s && !self.settings.rating_q && !self.settings.rating_e {
            self.dialog = Some(DialogState::Info {
                title: "No rating".to_string(),
                message: "Select at least one rating.".to_string(),
            });
            return;
        }
        let base = raw
            .unwrap_or_else(|| self.tag_query.clone())
            .trim()
            .to_string();
        let src_label = label.unwrap_or_else(|| {
            if base.is_empty() {
                "latest".to_string()
            } else {
                base.clone()
            }
        });
        let composed = compose_query(&base, &self.settings);
        self.begin(Source::new(kind, src_label, composed), new_tab);
        self.current_tab_mut().exhausted = false;
        self.load_next_page();
    }

    fn load_next_page(&mut self) {
        let (tab_id, tab_gen, page, query, skip) = {
            let tab = self.current_tab();
            (
                tab.id,
                tab.gen,
                tab.page,
                tab.source.query.clone(),
                tab.loading || tab.exhausted || tab.source.kind == SourceKind::Pool,
            )
        };
        if skip {
            return;
        }
        self.current_tab_mut().loading = true;
        self.set_status(format!("Loading page {page}…"), None);
        self.workers.submit_api(ApiTask::SearchPosts {
            client: self.client.clone(),
            tab_id,
            gen: tab_gen,
            query,
            page,
        });
    }

    fn open_pool(&mut self, _ctx: &Context, pool: Pool, new_tab: bool) {
        let display = pool.display_name();
        let query = format!("pool:{}", pool.id);
        self.begin(
            Source::new(SourceKind::Pool, display.clone(), query).with_pool(pool.clone()),
            new_tab,
        );
        let tab = self.current_tab_mut();
        tab.loading = true;
        let tab_id = tab.id;
        let tab_gen = tab.gen;
        self.set_status(format!("Loading pool {display}…"), None);
        self.workers.submit_api(ApiTask::LoadPoolPosts {
            client: self.client.clone(),
            tab_id,
            gen: tab_gen,
            pool,
        });
    }

    fn open_pool_by_id(&mut self, pool_id: u64, new_tab: bool) {
        self.set_status(format!("Fetching pool #{pool_id}…"), None);
        self.workers.submit_api(ApiTask::FetchPoolMetadata {
            client: self.client.clone(),
            pool_id,
            action: if new_tab {
                PoolMetaAction::OpenNewTab
            } else {
                PoolMetaAction::Open
            },
        });
    }

    fn download_pool_by_id(&mut self, pool_id: u64) {
        let tab = self.current_tab();
        if tab.source.kind == SourceKind::Pool
            && tab.source.pool.as_ref().map(|p| p.id) == Some(pool_id)
            && !tab.posts.is_empty()
        {
            let posts = tab.posts.clone();
            let src = tab.source.clone();
            self.enqueue(&posts, Some(&src));
            return;
        }
        self.set_status(format!("Fetching pool #{pool_id} for download…"), None);
        self.workers.submit_api(ApiTask::FetchPoolMetadata {
            client: self.client.clone(),
            pool_id,
            action: PoolMetaAction::Download,
        });
    }

    fn toggle_pool_in_bank(&mut self, pool: &Pool) -> bool {
        let added = self.spank_bank.toggle_pool(pool);
        self.refresh_bank_cache();
        let msg = if added {
            format!("Saved pool '{}' to Spank Bank", pool.display_name())
        } else {
            format!("Removed pool '{}' from Spank Bank", pool.display_name())
        };
        self.set_status(&msg, Some(3000));
        self.log(msg);
        added
    }

    fn toggle_pool_by_id_in_bank(&mut self, pool_id: u64) {
        if self.spank_bank.has_pool(pool_id) {
            self.spank_bank.remove_pool(pool_id);
            self.refresh_bank_cache();
            let msg = format!("Removed pool #{pool_id} from Spank Bank");
            self.set_status(&msg, Some(3000));
            self.log(msg);
        } else if let Some(pool) = self
            .current_tab()
            .source
            .pool
            .as_ref()
            .filter(|p| p.id == pool_id)
            .cloned()
        {
            self.toggle_pool_in_bank(&pool);
        } else {
            self.set_status(format!("Fetching pool #{pool_id} metadata…"), None);
            self.workers.submit_api(ApiTask::FetchPoolMetadata {
                client: self.client.clone(),
                pool_id,
                action: PoolMetaAction::ToggleBank,
            });
        }
    }

    fn open_artist(&mut self, name: &str, new_tab: bool) {
        let norm = SpankBank::norm_artist(name);
        if norm.is_empty() {
            return;
        }
        self.start_search(
            Some(norm.clone()),
            Some(norm.clone()),
            SourceKind::Artist,
            new_tab,
        );
        self.set_status(format!("Loading posts by {norm}…"), None);
    }

    fn toggle_artist_in_bank(&mut self, name: &str) -> bool {
        let norm = SpankBank::norm_artist(name);
        if norm.is_empty() {
            return false;
        }
        let added = self.spank_bank.toggle_artist(&norm);
        self.refresh_bank_cache();
        let msg = if added {
            format!("Saved artist '{norm}' to Spank Bank")
        } else {
            format!("Removed artist '{norm}' from Spank Bank")
        };
        self.set_status(&msg, Some(3000));
        self.log(msg);
        added
    }

    fn toggle_post_in_bank(&mut self, post: &Post) -> bool {
        let added = self.spank_bank.toggle(post);
        self.refresh_bank_cache();
        let msg = if added {
            format!("Saved post #{} to Spank Bank", post.id)
        } else {
            format!("Removed post #{} from Spank Bank", post.id)
        };
        self.set_status(&msg, Some(3000));
        self.log(msg);
        added
    }

    fn enqueue(&mut self, posts: &[Post], src_override: Option<&Source>) {
        if posts.is_empty() {
            self.set_status("Nothing to download.", Some(4000));
            return;
        }
        let src = src_override.unwrap_or(&self.current_tab().source);
        let base = self.effective_download_dir();
        let mode = &self.settings.organize_mode;
        let jobs = make_download_jobs(
            posts,
            src,
            &base,
            mode,
            self.settings.write_tags_file,
            self.settings.skip_downloaded,
            &self.bank_pools,
        );
        let count = jobs.len();
        self.downloads.enqueue(jobs);
        let label = organize_mode_label(mode);
        self.log(format!("Queued {count} post(s) → {base} [{label}]"));
    }

    fn navigate_back(&mut self, ctx: &Context) {
        let tab = self.current_tab_mut();
        let Some(prev) = tab.history_back.pop() else {
            return;
        };
        let curr = tab.snapshot();
        tab.history_forward.push(curr);
        self.restore_snapshot(ctx, prev);
    }

    fn navigate_forward(&mut self, ctx: &Context) {
        let tab = self.current_tab_mut();
        let Some(next) = tab.history_forward.pop() else {
            return;
        };
        let curr = tab.snapshot();
        tab.history_back.push(curr);
        self.restore_snapshot(ctx, next);
    }

    fn restore_snapshot(&mut self, ctx: &Context, snap: crate::ui::state::TabSnapshot) {
        self.stop_fetch.store(true, Ordering::Relaxed);
        self.workers.clear_pending_thumbs();

        self.pending_scroll_restore = Some(snap.scroll_offset);
        let dl_ids: HashSet<u64> = snap
            .posts
            .iter()
            .filter(|p| self.history.has(p.id))
            .map(|p| p.id)
            .collect();

        let (tab_id, tab_gen, posts_to_load) = {
            let tab = self.current_tab_mut();
            tab.gen += 1;
            tab.clear_posts();
            tab.source = snap.source;
            tab.page = snap.page;
            tab.exhausted = snap.exhausted;
            tab.hidden = snap.hidden;
            tab.loading = false;
            tab.scroll_offset = snap.scroll_offset;
            let added = tab.add_posts(snap.posts, &dl_ids);
            (tab.id, tab.gen, added)
        };

        for p in posts_to_load {
            if self.thumb_textures.contains_key(&p.id) {
                continue;
            }
            if let Some(mem) = self.image_cache.get_memory_thumb(p.id) {
                self.store_thumb_texture(ctx, p.id, &mem);
            } else if let Some(ref url) = p.preview_url {
                let cache_path = self.image_cache.thumb_path(p.id, &p.ext);
                self.workers.submit_thumb(
                    tab_id,
                    tab_gen,
                    p.id,
                    url.clone(),
                    cache_path,
                    self.image_cache.clone(),
                );
            }
        }
    }

    fn update_viewer_window_title(&self, ctx: &Context) {
        if let Some(ref v) = self.viewer {
            if let Some(p) = v.current_post() {
                let title = format!(
                    "Post #{} ({}/{}) - Full Screen View",
                    p.id,
                    v.index + 1,
                    v.posts.len()
                );
                ctx.send_viewport_cmd(ViewportCommand::Title(title));
            }
        }
    }

    fn close_viewer_window(&mut self, ctx: &Context) {
        self.viewer = None;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
        ctx.send_viewport_cmd(ViewportCommand::Title("E621 Cruiser".to_string()));
    }

    fn open_viewer(
        &mut self,
        ctx: &Context,
        post: Post,
        posts_override: Option<Vec<Post>>,
        start_slideshow: bool,
        shuffle: bool,
    ) {
        let mut all_posts = posts_override.unwrap_or_else(|| self.current_tab().posts.clone());
        if all_posts.is_empty() {
            all_posts.push(post.clone());
        }
        let idx = all_posts
            .iter()
            .position(|p| p.id == post.id)
            .unwrap_or(0);
        let interval = self.settings.slideshow_interval;
        let state = ViewerState::new(all_posts, idx, interval, start_slideshow, shuffle);
        let active = state.current_post().cloned().unwrap_or(post);
        self.viewer = Some(state);
        self.update_viewer_window_title(ctx);
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
        self.load_preview_image(ctx, &active, true);
    }

    fn viewer_image_url_and_ext(post: &Post) -> (Option<String>, &str) {
        if post.is_non_image_media() {
            let sample_ok = post
                .sample_url
                .as_ref()
                .filter(|u| !has_video_ext(u))
                .cloned();
            let url = sample_ok.or_else(|| post.preview_url.clone());
            let ext = url
                .as_deref()
                .map(|u| {
                    if u.to_ascii_lowercase().contains(".webp") {
                        "webp"
                    } else {
                        "jpg"
                    }
                })
                .unwrap_or("jpg");
            (url, ext)
        } else {
            let url = post
                .sample_url
                .clone()
                .or_else(|| post.file_url.clone())
                .or_else(|| post.preview_url.clone());
            (url, post.ext.as_str())
        }
    }

    fn load_preview_image(&mut self, ctx: &Context, post: &Post, for_viewer: bool) {
        if self.preview_textures.contains_key(&post.id) {
            if for_viewer {
                self.prefetch_next_slide(ctx);
            }
            return;
        }
        if let Some(mem) = self.image_cache.get_memory_preview(post.id) {
            self.store_preview_texture(ctx, post.id, &mem);
            if for_viewer {
                self.prefetch_next_slide(ctx);
            }
            return;
        }

        let mut local_path = self
            .history
            .path_for(post.id)
            .map(std::path::PathBuf::from);
        if local_path.is_none() && !self.settings.download_dir.is_empty() {
            let candidate = expand_tilde(&self.settings.download_dir)
                .join(format!("{}.{}", post.id, post.ext));
            if candidate.exists()
                && fs::metadata(&candidate)
                    .map(|m| m.len() > 0)
                    .unwrap_or(false)
            {
                self.history.add(post, &candidate);
                local_path = Some(candidate);
            }
        }

        let (url_opt, cache_ext) = Self::viewer_image_url_and_ext(post);
        let cache_path = self.image_cache.preview_path(post.id, cache_ext);

        let local_valid = local_path
            .as_ref()
            .map(|p| !crate::workers::is_non_image_path(p) && p.exists())
            .unwrap_or(false);
        let cache_valid = !crate::workers::is_non_image_path(&cache_path)
            && cache_path.exists()
            && fs::metadata(&cache_path)
                .map(|m| m.len() > 0)
                .unwrap_or(false);

        if url_opt.is_none() && !local_valid && !cache_valid {
            if for_viewer {
                if let Some(ref mut v) = self.viewer {
                    if v.current_post().map(|p| p.id) == Some(post.id) {
                        v.loading_error =
                            Some("No image URL (post may require login in Settings).".to_string());
                    }
                }
            }
            return;
        }

        self.workers.submit_preview(
            post.id,
            url_opt.unwrap_or_default(),
            cache_path,
            local_path,
            for_viewer,
            false,
            self.image_cache.clone(),
        );
    }

    fn prefetch_next_slide(&self, _ctx: &Context) {
        let Some(ref v) = self.viewer else {
            return;
        };
        if v.posts.len() <= 1 {
            return;
        }
        let next_idx = (v.index + 1) % v.posts.len();
        let next_post = &v.posts[next_idx];
        if self.preview_textures.contains_key(&next_post.id)
            || self.image_cache.get_memory_preview(next_post.id).is_some()
        {
            return;
        }
        let local_path = self
            .history
            .path_for(next_post.id)
            .map(std::path::PathBuf::from);
        let (url_opt, cache_ext) = Self::viewer_image_url_and_ext(next_post);
        let cache_path = self.image_cache.preview_path(next_post.id, cache_ext);
        let cache_exists = cache_path.exists()
            && fs::metadata(&cache_path)
                .map(|m| m.len() > 0)
                .unwrap_or(false);
        if url_opt.is_some() || local_path.is_some() || cache_exists {
            self.workers.submit_preview(
                next_post.id,
                url_opt.unwrap_or_default(),
                cache_path,
                local_path,
                true,
                true,
                self.image_cache.clone(),
            );
        }
    }

    fn toggle_tag_in_search(&mut self, tag: &str) {
        self.left_tab = LeftTab::Search;
        let mut tokens: Vec<String> = self
            .tag_query
            .split_whitespace()
            .map(ToOwned::to_owned)
            .collect();
        let neg = format!("-{tag}");
        if let Some(pos) = tokens.iter().position(|t| t == tag) {
            tokens.remove(pos);
            self.set_status(format!("Removed “{tag}” from search"), Some(3000));
        } else if let Some(pos) = tokens.iter().position(|t| t == &neg) {
            tokens.remove(pos);
            tokens.push(tag.to_string());
            self.set_status(format!("Added “{tag}” to search"), Some(3000));
        } else {
            tokens.push(tag.to_string());
            self.set_status(format!("Added “{tag}” to search"), Some(3000));
        }
        self.tag_query = if tokens.is_empty() {
            String::new()
        } else {
            format!("{} ", tokens.join(" "))
        };
    }

    fn exclude_tag_in_search(&mut self, tag: &str) {
        self.left_tab = LeftTab::Search;
        let mut tokens: Vec<String> = self
            .tag_query
            .split_whitespace()
            .map(ToOwned::to_owned)
            .collect();
        let neg = format!("-{tag}");
        if let Some(pos) = tokens.iter().position(|t| t == &neg) {
            tokens.remove(pos);
            self.set_status(format!("Removed exclusion “-{tag}”"), Some(3000));
        } else {
            tokens.retain(|t| t != tag);
            tokens.push(neg);
            self.set_status(format!("Excluded “-{tag}” from search"), Some(3000));
        }
        self.tag_query = if tokens.is_empty() {
            String::new()
        } else {
            format!("{} ", tokens.join(" "))
        };
    }

    fn search_only_tag(&mut self, tag: &str) {
        self.left_tab = LeftTab::Search;
        self.tag_query = format!("{tag} ");
        self.start_search(None, None, SourceKind::Search, false);
    }

    fn prompt_login(&mut self, action_name: &str) -> bool {
        if self.client.authenticated() {
            return true;
        }
        self.dialog = Some(DialogState::Confirm {
            title: "e621 Login Required".to_string(),
            message: format!(
                "{action_name} on e621 requires logging in with your e621 Username and API key so the site can register your action and credit the artist.\n\nWould you like to open Settings now to enter your credentials?"
            ),
            yes_label: "Yes".to_string(),
            no_label: "No".to_string(),
            action: ConfirmAction::OpenSettingsForLogin,
        });
        false
    }

    fn handle_actions(&mut self, ctx: &Context, actions: Vec<UiAction>) {
        for action in actions {
            match action {
                UiAction::NewTab => {
                    let id = self.next_tab_id;
                    self.next_tab_id += 1;
                    let mut tab = ResultTab::new(id, Source::latest());
                    tab.custom_title = Some("New Tab".to_string());
                    self.tabs.push(tab);
                    self.active_tab = self.tabs.len() - 1;
                    self.left_tab = LeftTab::Search;
                    self.focus_search_requested = true;
                }
                UiAction::SwitchTab(idx) => {
                    if idx < self.tabs.len() {
                        self.active_tab = idx;
                        self.pending_scroll_restore = Some(self.tabs[idx].scroll_offset);
                    }
                }
                UiAction::CloseTab(idx) => {
                    if idx < self.tabs.len() {
                        if self.tabs.len() <= 1 {
                            self.tabs[0].history_back.clear();
                            self.tabs[0].history_forward.clear();
                            self.begin(Source::latest(), false);
                            self.tabs[0].custom_title = Some("Search".to_string());
                        } else {
                            self.tabs.remove(idx);
                            if self.active_tab >= self.tabs.len() {
                                self.active_tab = self.tabs.len() - 1;
                            }
                            self.pending_scroll_restore = Some(self.current_tab().scroll_offset);
                        }
                    }
                }
                UiAction::NavigateBack => self.navigate_back(ctx),
                UiAction::NavigateForward => self.navigate_forward(ctx),

                UiAction::StartSearch {
                    raw,
                    label,
                    kind,
                    new_tab,
                } => self.start_search(raw, label, kind, new_tab),
                UiAction::SaveFiltersToSettings => {
                    self.blacklist = Blacklist::new(&self.settings.full_blacklist());
                    self.settings.save();
                }
                UiAction::SelectCommonBlacklist => {
                    self.settings.quick_blacklist = COMMON_QUICK_BLACKLIST
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect();
                    self.blacklist = Blacklist::new(&self.settings.full_blacklist());
                    self.settings.save();
                }
                UiAction::ClearQuickBlacklist => {
                    self.settings.quick_blacklist.clear();
                    self.blacklist = Blacklist::new(&self.settings.full_blacklist());
                    self.settings.save();
                }
                UiAction::SaveSearch => {
                    let q = self.tag_query.trim().to_string();
                    if !q.is_empty() && !self.settings.saved_searches.contains(&q) {
                        self.settings.saved_searches.push(q);
                        self.settings.save();
                    }
                }
                UiAction::RemoveSavedSearch(idx) => {
                    if idx < self.settings.saved_searches.len() {
                        self.settings.saved_searches.remove(idx);
                        self.selected_saved_idx = None;
                        self.settings.save();
                    }
                }
                UiAction::RunSavedSearch(q) => {
                    self.tag_query = q;
                    self.start_search(None, None, SourceKind::Search, false);
                }
                UiAction::TriggerAutocomplete(tok) => {
                    self.workers
                        .submit_autocomplete(self.client.clone(), tok);
                }

                UiAction::SearchPools => {
                    let q = self.pool_query.trim().to_string();
                    if !q.is_empty() {
                        self.pool_results.clear();
                        self.selected_pool_idx = None;
                        self.set_status("Searching pools…", None);
                        self.workers.submit_api(ApiTask::SearchPools {
                            client: self.client.clone(),
                            query: q,
                        });
                    }
                }
                UiAction::OpenPool { pool, new_tab } => {
                    if self.viewer.is_some() {
                        self.close_viewer_window(ctx);
                    }
                    self.open_pool(ctx, pool, new_tab);
                }
                UiAction::OpenPoolById { pool_id, new_tab } => {
                    if self.viewer.is_some() {
                        self.close_viewer_window(ctx);
                    }
                    self.open_pool_by_id(pool_id, new_tab);
                }
                UiAction::DownloadPool(pool) => {
                    let tab = self.current_tab();
                    if tab.source.kind == SourceKind::Pool
                        && tab.source.pool.as_ref().map(|p| p.id) == Some(pool.id)
                        && !tab.posts.is_empty()
                    {
                        let posts = tab.posts.clone();
                        let src = tab.source.clone();
                        self.enqueue(&posts, Some(&src));
                    } else {
                        self.handle_event(
                            ctx,
                            BgEvent::PoolMetadataLoaded {
                                pool,
                                action: PoolMetaAction::Download,
                            },
                        );
                    }
                }
                UiAction::DownloadPoolById(pid) => self.download_pool_by_id(pid),
                UiAction::TogglePoolInBank(pool) => {
                    self.toggle_pool_in_bank(&pool);
                }
                UiAction::TogglePoolByIdInBank(pid) => self.toggle_pool_by_id_in_bank(pid),
                UiAction::LoadFavorites => {
                    let u = self.fav_user.trim().to_string();
                    let user = if u.is_empty() {
                        self.settings.username.trim().to_string()
                    } else {
                        u
                    };
                    if user.is_empty() {
                        self.dialog = Some(DialogState::Info {
                            title: "No user".to_string(),
                            message: "Enter a username or set your account in Settings."
                                .to_string(),
                        });
                    } else {
                        self.settings.favorites_user = self.fav_user.trim().to_string();
                        self.settings.save();
                        self.start_search(
                            Some(format!("fav:{user}")),
                            Some(format!("favorites_{user}")),
                            SourceKind::Favorites,
                            false,
                        );
                    }
                }

                UiAction::OpenArtist { name, new_tab } => {
                    if self.viewer.is_some() {
                        self.close_viewer_window(ctx);
                    }
                    self.open_artist(&name, new_tab);
                }
                UiAction::ToggleArtistInBank(name) => {
                    self.toggle_artist_in_bank(&name);
                }
                UiAction::AddArtistToBankFromEdit => {
                    let norm = SpankBank::norm_artist(&self.bank_artist_add);
                    if !norm.is_empty() {
                        if !self.spank_bank.has_artist(&norm) {
                            self.toggle_artist_in_bank(&norm);
                        }
                        self.bank_artist_add.clear();
                    }
                }
                UiAction::OpenSavedArtistsFeed { new_tab } => {
                    if self.bank_artists.is_empty() {
                        self.dialog = Some(DialogState::Info {
                            title: "No saved artists".to_string(),
                            message: "No artists in the Spank Bank yet.\nRight-click an artist tag or use '+ Bank Artist' on any post.".to_string(),
                        });
                    } else {
                        let used: Vec<&String> =
                            self.bank_artists.iter().take(MAX_FEED_ARTISTS).collect();
                        let used_len = used.len();
                        let raw = if used_len == 1 {
                            used[0].clone()
                        } else {
                            used.iter()
                                .map(|a| format!("~{a}"))
                                .collect::<Vec<_>>()
                                .join(" ")
                        };
                        self.start_search(
                            Some(raw),
                            Some("saved_artists".to_string()),
                            SourceKind::ArtistFeed,
                            new_tab,
                        );
                        if self.bank_artists.len() > used_len {
                            self.set_status(
                                format!(
                                    "e621 limits tags per search: feed shows the {used_len} most recently saved artists"
                                ),
                                Some(6000),
                            );
                        }
                    }
                }
                UiAction::PlayArtistSlideshow { name, shuffle } => {
                    self.set_status(format!("Loading posts by {name} for slideshow…"), None);
                    self.workers.submit_api(ApiTask::LoadSlideshowPosts {
                        client: self.client.clone(),
                        query: compose_query(&name, &self.settings),
                        pool: None,
                        shuffle,
                        title: name,
                    });
                }
                UiAction::RemoveSelectedBankArtists => {
                    let ids: Vec<String> = self.selected_bank_artists.drain().collect();
                    if !ids.is_empty() {
                        let cnt = ids.len();
                        for name in ids {
                            self.spank_bank.remove_artist(&name);
                        }
                        self.refresh_bank_cache();
                        self.set_status(
                            format!("Removed {cnt} artist(s) from Spank Bank"),
                            Some(3000),
                        );
                    }
                }
                UiAction::ClearBankArtistsPrompt => {
                    let cnt = self.bank_artists.len();
                    if cnt > 0 {
                        self.dialog = Some(DialogState::Confirm {
                            title: "Clear Saved Artists".to_string(),
                            message: format!(
                                "Are you sure you want to clear all {cnt} saved artists?"
                            ),
                            yes_label: "Yes".to_string(),
                            no_label: "No".to_string(),
                            action: ConfirmAction::ClearBankArtists,
                        });
                    }
                }

                UiAction::OpenSpankBank { new_tab } => {
                    let posts = self.spank_bank.get_all();
                    if posts.is_empty() {
                        self.dialog = Some(DialogState::Info {
                            title: "Spank Bank empty".to_string(),
                            message: "No posts have been added to the Spank Bank yet.\nClick '+ Bank' on any post or press 'B' in preview to save it.".to_string(),
                        });
                    } else {
                        let src = Source::new(
                            SourceKind::SpankBank,
                            "Spank Bank",
                            "spank_bank",
                        );
                        self.begin(src, new_tab);
                        let tab_idx = self.active_tab;
                        let count = posts.len();
                        self.show_posts_in_tab(ctx, tab_idx, &posts);
                        self.tabs[tab_idx].exhausted = true;
                        self.set_status(
                            format!("Loaded {count} posts from Spank Bank"),
                            Some(3000),
                        );
                    }
                }
                UiAction::PlayBankSlideshow { shuffle } => {
                    let mut posts = self.spank_bank.get_all();
                    if posts.is_empty() {
                        self.dialog = Some(DialogState::Info {
                            title: "Spank Bank empty".to_string(),
                            message: "No posts in Spank Bank to play.".to_string(),
                        });
                    } else {
                        if shuffle {
                            let mut rng = rand::thread_rng();
                            posts.shuffle(&mut rng);
                        }
                        let first = posts[0].clone();
                        self.open_viewer(ctx, first, Some(posts), true, shuffle);
                    }
                }
                UiAction::PlayBankPoolSlideshow { pool, shuffle } => {
                    let tab = self.current_tab();
                    if tab.source.kind == SourceKind::Pool
                        && tab.source.pool.as_ref().map(|p| p.id) == Some(pool.id)
                        && !tab.posts.is_empty()
                    {
                        let mut posts = tab.posts.clone();
                        if shuffle {
                            let mut rng = rand::thread_rng();
                            posts.shuffle(&mut rng);
                        }
                        let first = posts[0].clone();
                        self.open_viewer(ctx, first, Some(posts), true, shuffle);
                    } else {
                        let title = pool.display_name();
                        self.set_status(format!("Loading pool '{title}' for slideshow…"), None);
                        self.workers.submit_api(ApiTask::LoadSlideshowPosts {
                            client: self.client.clone(),
                            query: String::new(),
                            pool: Some(pool),
                            shuffle,
                            title,
                        });
                    }
                }
                UiAction::DownloadSelectedBankPosts => {
                    let posts: Vec<Post> = self
                        .bank_posts
                        .iter()
                        .filter(|p| self.selected_bank_post_ids.contains(&p.id))
                        .cloned()
                        .collect();
                    if posts.is_empty() {
                        self.dialog = Some(DialogState::Info {
                            title: "No posts selected".to_string(),
                            message: "Select one or more posts in the Spank Bank posts list first, or click 'Download All Posts'.".to_string(),
                        });
                    } else {
                        let src = Source::new(
                            SourceKind::SpankBank,
                            "spank_bank",
                            "spank_bank",
                        );
                        self.enqueue(&posts, Some(&src));
                    }
                }
                UiAction::DownloadAllBankPosts => {
                    let posts = self.spank_bank.get_all();
                    if posts.is_empty() {
                        self.dialog = Some(DialogState::Info {
                            title: "Spank Bank empty".to_string(),
                            message: "No saved posts in the Spank Bank to download.".to_string(),
                        });
                    } else {
                        let src = Source::new(
                            SourceKind::SpankBank,
                            "spank_bank",
                            "spank_bank",
                        );
                        self.enqueue(&posts, Some(&src));
                    }
                }
                UiAction::DownloadSelectedBankPools => {
                    let pools: Vec<Pool> = self
                        .bank_pools
                        .iter()
                        .filter(|p| self.selected_bank_pool_ids.contains(&p.id))
                        .cloned()
                        .collect();
                    if pools.is_empty() {
                        if let Some(pid) = self.last_bank_pool_id {
                            self.download_pool_by_id(pid);
                        } else {
                            self.dialog = Some(DialogState::Info {
                                title: "No pool selected".to_string(),
                                message: "Select one or more pools in the saved pools list first."
                                    .to_string(),
                            });
                        }
                    } else {
                        for pool in pools {
                            self.handle_event(
                                ctx,
                                BgEvent::PoolMetadataLoaded {
                                    pool,
                                    action: PoolMetaAction::Download,
                                },
                            );
                        }
                    }
                }
                UiAction::DownloadAllBankPools => {
                    let pools = self.spank_bank.get_all_pools();
                    if pools.is_empty() {
                        self.dialog = Some(DialogState::Info {
                            title: "No saved pools".to_string(),
                            message: "No pools saved in the Spank Bank to download.".to_string(),
                        });
                    } else {
                        for pool in pools {
                            self.handle_event(
                                ctx,
                                BgEvent::PoolMetadataLoaded {
                                    pool,
                                    action: PoolMetaAction::Download,
                                },
                            );
                        }
                    }
                }
                UiAction::DownloadEntireBank => {
                    let posts = self.spank_bank.get_all();
                    let pools = self.spank_bank.get_all_pools();
                    if posts.is_empty() && pools.is_empty() {
                        self.dialog = Some(DialogState::Info {
                            title: "Spank Bank empty".to_string(),
                            message: "No saved posts or pools in the Spank Bank to download."
                                .to_string(),
                        });
                    } else {
                        if !posts.is_empty() {
                            let src = Source::new(
                                SourceKind::SpankBank,
                                "spank_bank",
                                "spank_bank",
                            );
                            self.enqueue(&posts, Some(&src));
                        }
                        for pool in pools {
                            self.handle_event(
                                ctx,
                                BgEvent::PoolMetadataLoaded {
                                    pool,
                                    action: PoolMetaAction::Download,
                                },
                            );
                        }
                    }
                }
                UiAction::RemoveSelectedBankPosts => {
                    let ids: Vec<u64> = self.selected_bank_post_ids.drain().collect();
                    if !ids.is_empty() {
                        let cnt = ids.len();
                        for pid in ids {
                            self.spank_bank.remove(pid);
                        }
                        self.refresh_bank_cache();
                        self.set_status(
                            format!("Removed {cnt} post(s) from Spank Bank"),
                            Some(3000),
                        );
                    }
                }
                UiAction::ClearBankPostsPrompt => {
                    let cnt = self.bank_posts.len();
                    if cnt > 0 {
                        self.dialog = Some(DialogState::Confirm {
                            title: "Clear Spank Bank".to_string(),
                            message: format!(
                                "Are you sure you want to clear all {cnt} saved posts?"
                            ),
                            yes_label: "Yes".to_string(),
                            no_label: "No".to_string(),
                            action: ConfirmAction::ClearBankPosts,
                        });
                    }
                }
                UiAction::RemoveSelectedBankPools => {
                    let ids: Vec<u64> = self.selected_bank_pool_ids.drain().collect();
                    if !ids.is_empty() {
                        let cnt = ids.len();
                        for pid in ids {
                            self.spank_bank.remove_pool(pid);
                        }
                        self.refresh_bank_cache();
                        self.set_status(
                            format!("Removed {cnt} pool(s) from Spank Bank"),
                            Some(3000),
                        );
                    }
                }
                UiAction::ClearBankPoolsPrompt => {
                    let cnt = self.bank_pools.len();
                    if cnt > 0 {
                        self.dialog = Some(DialogState::Confirm {
                            title: "Clear Saved Pools".to_string(),
                            message: format!(
                                "Are you sure you want to clear all {cnt} saved pools?"
                            ),
                            yes_label: "Yes".to_string(),
                            no_label: "No".to_string(),
                            action: ConfirmAction::ClearBankPools,
                        });
                    }
                }
                UiAction::TogglePostInBank(post) => {
                    self.toggle_post_in_bank(&post);
                }
                UiAction::ToggleCurrentSourceInBank => {
                    let src = self.current_tab().source.clone();
                    match src.kind {
                        SourceKind::Pool => {
                            if let Some(pool) = src.pool {
                                self.toggle_pool_in_bank(&pool);
                            }
                        }
                        SourceKind::Artist => {
                            self.toggle_artist_in_bank(&src.label);
                        }
                        _ => {}
                    }
                }

                UiAction::ToggleTagInSearch(tag) => self.toggle_tag_in_search(&tag),
                UiAction::ExcludeTagInSearch(tag) => self.exclude_tag_in_search(&tag),
                UiAction::SearchOnlyTag(tag) => self.search_only_tag(&tag),
                UiAction::CopyText(txt) => ctx.copy_text(txt),

                UiAction::ToggleFavorite(post) => {
                    if self.prompt_login("Favoriting / liking posts") {
                        let make_fav = !post.is_favorited;
                        let verb = if make_fav {
                            "Favoriting"
                        } else {
                            "Removing favorite from"
                        };
                        self.set_status(format!("{verb} post #{} on e621…", post.id), None);
                        self.workers.submit_api(ApiTask::ToggleFavorite {
                            client: self.client.clone(),
                            post_id: post.id,
                            make_favorite: make_fav,
                        });
                    }
                }
                UiAction::ToggleUpvote(post) => {
                    if self.prompt_login("Voting on posts") {
                        let make_up = post.own_vote != 1;
                        let verb = if make_up {
                            "Upvoting"
                        } else {
                            "Removing upvote from"
                        };
                        self.set_status(format!("{verb} post #{} on e621…", post.id), None);
                        self.workers.submit_api(ApiTask::ToggleUpvote {
                            client: self.client.clone(),
                            post_id: post.id,
                            make_upvote: make_up,
                        });
                    }
                }
                UiAction::OpenViewer {
                    post,
                    posts,
                    start_slideshow,
                    shuffle,
                } => self.open_viewer(ctx, post, posts, start_slideshow, shuffle),
                UiAction::CloseViewer => {
                    self.close_viewer_window(ctx);
                }
                UiAction::ViewerPostChanged(post) => {
                    self.current_tab_mut().active_post_id = Some(post.id);
                    self.update_viewer_window_title(ctx);
                    self.load_preview_image(ctx, &post, true);
                }
                UiAction::ViewerIntervalChanged(val) => {
                    self.settings.slideshow_interval = val;
                    self.settings.save();
                }
                UiAction::StartTabSlideshow => {
                    let posts = self.current_tab().posts.clone();
                    if posts.is_empty() {
                        self.dialog = Some(DialogState::Info {
                            title: "No posts".to_string(),
                            message: "No posts in the current view to play in a slideshow."
                                .to_string(),
                        });
                    } else {
                        let first = posts[0].clone();
                        self.open_viewer(ctx, first, Some(posts), true, false);
                    }
                }

                UiAction::LoadNextPage => self.load_next_page(),
                UiAction::GridPostSelected {
                    post,
                    ctrl,
                    shift,
                    index,
                } => {
                    let tab = self.current_tab_mut();
                    if shift {
                        if let Some(anchor) = tab.last_clicked_idx {
                            let (lo, hi) = if anchor <= index {
                                (anchor, index)
                            } else {
                                (index, anchor)
                            };
                            for i in lo..=hi {
                                if let Some(p) = tab.posts.get(i) {
                                    tab.selected_ids.insert(p.id);
                                }
                            }
                        } else {
                            tab.selected_ids.insert(post.id);
                        }
                    } else if ctrl {
                        if !tab.selected_ids.remove(&post.id) {
                            tab.selected_ids.insert(post.id);
                        }
                        tab.last_clicked_idx = Some(index);
                    } else {
                        tab.selected_ids.clear();
                        tab.selected_ids.insert(post.id);
                        tab.last_clicked_idx = Some(index);
                    }
                    tab.active_post_id = Some(post.id);
                }
                UiAction::SelectAllPosts => self.current_tab_mut().select_all(),
                UiAction::SelectNoPosts => self.current_tab_mut().clear_selection(),
                UiAction::DownloadPosts(posts) => self.enqueue(&posts, None),
                UiAction::DownloadSelectedPosts => {
                    let posts = self.current_tab().selected_posts();
                    if posts.is_empty() {
                        self.dialog = Some(DialogState::Info {
                            title: "Nothing selected".to_string(),
                            message:
                                "Select thumbnails first (Ctrl/Shift-click, or Select all)."
                                    .to_string(),
                        });
                    } else {
                        self.enqueue(&posts, None);
                    }
                }
                UiAction::DownloadAllResults => {
                    let tab = self.current_tab();
                    if tab.exhausted || tab.source.kind == SourceKind::Pool {
                        let posts = tab.posts.clone();
                        self.enqueue(&posts, None);
                    } else {
                        let tab_id = tab.id;
                        let gen = tab.gen;
                        let src = tab.source.clone();
                        let shown = tab.posts.clone();
                        let start_page = tab.page;
                        self.stop_fetch.store(false, Ordering::Relaxed);
                        let stop_flag = Arc::clone(&self.stop_fetch);
                        self.set_status("Collecting all results…", None);
                        self.workers.submit_api(ApiTask::CollectAllPosts {
                            client: self.client.clone(),
                            tab_id,
                            gen,
                            source: src,
                            shown,
                            start_page,
                            stop_flag,
                        });
                    }
                }
                UiAction::DownloadCurrentPoolOrBank => {
                    let src = self.current_tab().source.clone();
                    match src.kind {
                        SourceKind::Pool => {
                            if let Some(p) = src.pool {
                                self.download_pool_by_id(p.id);
                            }
                        }
                        SourceKind::SpankBank => {
                            self.handle_actions(ctx, vec![UiAction::DownloadEntireBank]);
                        }
                        _ => {
                            self.dialog = Some(DialogState::Info {
                                title: "Not in a pool".to_string(),
                                message: "The current view is not a pool.".to_string(),
                            });
                        }
                    }
                }
                UiAction::CancelDownloads => {
                    self.stop_fetch.store(true, Ordering::Relaxed);
                    self.downloads.cancel();
                    self.log("Downloads cancelled.");
                }
                UiAction::BrowseDownloadDir => {
                    let start = expand_tilde(&self.effective_download_dir());
                    self.workers.pick_folder_async(Some(start));
                }
                UiAction::OpenDownloadDir => {
                    let path = expand_tilde(&self.effective_download_dir());
                    let _ = fs::create_dir_all(&path);
                    let _ = open::that(&path);
                }
                UiAction::OrganizeModeChanged | UiAction::SaveTagsFileToggled => {
                    self.settings.save();
                }

                UiAction::OpenSettings => {
                    self.settings_modal = Some(SettingsModalState::from_settings(
                        &self.settings,
                        self.history.len(),
                    ));
                }
                UiAction::OpenFirstRunSplash => {
                    self.first_run_splash =
                        Some(FirstRunSplashState::from_settings(&self.settings, true));
                }
                UiAction::TestConnectionManual => {
                    self.set_status("Testing connection to e621…", None);
                    self.workers.submit_api(ApiTask::CheckConnection {
                        client: self.client.clone(),
                        manual: true,
                    });
                }
                UiAction::TestConnectionInModal => {
                    if let Some(ref mut modal) = self.settings_modal {
                        modal.testing = true;
                        modal.test_status = None;
                        let test_client = E621Client::new(
                            &modal.username,
                            &modal.api_key,
                            Duration::from_secs(1),
                        );
                        self.workers
                            .submit_api(ApiTask::TestSettingsConnection { client: test_client });
                    }
                }
                UiAction::TestConnectionInSplash => {
                    if let Some(ref mut splash) = self.first_run_splash {
                        splash.testing = true;
                        splash.test_status = None;
                        let test_client = E621Client::new(
                            splash.username.trim(),
                            splash.api_key.trim(),
                            Duration::from_secs(1),
                        );
                        self.workers
                            .submit_api(ApiTask::TestSettingsConnection { client: test_client });
                    }
                }
                UiAction::SaveSettingsModal => {
                    if let Some(modal) = self.settings_modal.take() {
                        modal.apply_to(&mut self.settings);
                        if !self.settings.username.is_empty() && !self.settings.api_key.is_empty() {
                            self.settings.first_run_complete = true;
                        }
                        if modal.clear_history_requested {
                            self.history.clear();
                            for tab in &mut self.tabs {
                                tab.downloaded_ids.clear();
                            }
                        }
                        self.settings.save();
                        self.apply_settings();
                        self.log("Settings saved.");
                    }
                }
                UiAction::CancelSettingsModal => {
                    self.settings_modal = None;
                }
                UiAction::SaveFirstRunSplash => {
                    if let Some(splash) = self.first_run_splash.take() {
                        splash.apply_to(&mut self.settings);
                        self.settings.save();
                        self.apply_settings();
                        self.log(format!(
                            "e621 credentials saved for {}.",
                            self.settings.username
                        ));
                        self.initial_connect_done = true;
                        self.set_status("Verifying e621 credentials…", None);
                        self.workers.submit_api(ApiTask::CheckConnection {
                            client: self.client.clone(),
                            manual: false,
                        });
                    }
                }
                UiAction::SkipFirstRunSplash => {
                    self.first_run_splash = None;
                    self.settings.first_run_complete = true;
                    self.settings.save();
                    self.apply_settings();
                    self.log("Skipped e621 API key setup (browsing anonymously).");
                }
                UiAction::CancelFirstRunSplash => {
                    self.first_run_splash = None;
                }
                UiAction::OpenUrl(url) => {
                    let _ = open::that(url);
                }
                UiAction::OpenPath(path) => {
                    let _ = open::that(path);
                }
                UiAction::RequestQuit => {
                    if self.downloads.busy() {
                        self.dialog = Some(DialogState::Confirm {
                            title: "Downloads running".to_string(),
                            message: "Downloads are still running. Quit anyway?".to_string(),
                            yes_label: "Yes".to_string(),
                            no_label: "No".to_string(),
                            action: ConfirmAction::QuitWhileDownloading,
                        });
                    } else {
                        self.prepare_shutdown(ctx);
                        self.allow_close = true;
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                }
            }
        }
    }

    fn prepare_shutdown(&mut self, ctx: &Context) {
        self.stop_fetch.store(true, Ordering::Relaxed);
        self.workers.stop();
        self.downloads.cancel();
        let size = ctx.screen_rect().size();
        if size.x > 200.0 && size.y > 200.0 {
            self.settings.window_width = Some(size.x);
            self.settings.window_height = Some(size.y);
        }
        self.settings.save();
    }

    fn handle_global_shortcuts(&mut self, ctx: &Context, actions: &mut Vec<UiAction>) {
        if self.first_run_splash.is_some()
            || self.viewer.is_some()
            || self.settings_modal.is_some()
            || self.dialog.is_some()
        {
            return;
        }
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND, Key::F) {
                self.left_tab = LeftTab::Search;
                self.focus_search_requested = true;
            } else if i.consume_key(Modifiers::COMMAND, Key::T) {
                actions.push(UiAction::NewTab);
            } else if i.consume_key(Modifiers::COMMAND, Key::W) {
                actions.push(UiAction::CloseTab(self.active_tab));
            } else if i.consume_key(Modifiers::COMMAND, Key::D) {
                actions.push(UiAction::DownloadSelectedPosts);
            } else if i.consume_key(Modifiers::COMMAND, Key::Comma) {
                actions.push(UiAction::OpenSettings);
            } else if i.consume_key(Modifiers::COMMAND, Key::Q) {
                actions.push(UiAction::RequestQuit);
            } else if i.consume_key(Modifiers::ALT, Key::ArrowLeft) {
                actions.push(UiAction::NavigateBack);
            } else if i.consume_key(Modifiers::ALT, Key::ArrowRight) {
                actions.push(UiAction::NavigateForward);
            }
        });
    }

    fn show_dialog(&mut self, ctx: &Context, actions: &mut Vec<UiAction>) {
        let Some(dlg) = self.dialog.clone() else {
            return;
        };
        let mut close_dlg = false;
        let mut confirmed_action: Option<ConfirmAction> = None;

        match dlg {
            DialogState::Info { title, message } => {
                let mut open = true;
                egui::Window::new(title)
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(300.0);
                        ui.label(message);
                        ui.add_space(8.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("OK").clicked() {
                                close_dlg = true;
                            }
                        });
                    });
                if !open {
                    close_dlg = true;
                }
            }
            DialogState::Confirm {
                title,
                message,
                yes_label,
                no_label,
                action,
            } => {
                let mut open = true;
                egui::Window::new(title)
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(320.0);
                        ui.label(message);
                        ui.add_space(10.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(&yes_label).clicked() {
                                confirmed_action = Some(action.clone());
                                close_dlg = true;
                            }
                            if ui.button(&no_label).clicked() {
                                close_dlg = true;
                            }
                        });
                    });
                if !open {
                    close_dlg = true;
                }
            }
        }

        if close_dlg {
            self.dialog = None;
        }
        if let Some(act) = confirmed_action {
            match act {
                ConfirmAction::ClearBankPosts => {
                    self.spank_bank.clear();
                    self.refresh_bank_cache();
                    self.set_status("Cleared Spank Bank", Some(3000));
                }
                ConfirmAction::ClearBankPools => {
                    self.spank_bank.clear_pools();
                    self.refresh_bank_cache();
                    self.set_status("Cleared all saved pools from Spank Bank", Some(3000));
                }
                ConfirmAction::ClearBankArtists => {
                    self.spank_bank.clear_artists();
                    self.refresh_bank_cache();
                    self.set_status("Cleared all saved artists from Spank Bank", Some(3000));
                }
                ConfirmAction::DownloadAllResults { source, posts } => {
                    self.enqueue(&posts, Some(&source));
                }
                ConfirmAction::QuitWhileDownloading => {
                    self.prepare_shutdown(ctx);
                    self.allow_close = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                ConfirmAction::OpenSettingsForLogin => {
                    actions.push(UiAction::OpenSettings);
                }
            }
        }
    }
}

fn has_video_ext(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let path_part = lower.split('?').next().unwrap_or(&lower);
    path_part.ends_with(".webm") || path_part.ends_with(".mp4") || path_part.ends_with(".swf")
}

impl eframe::App for E621CruiserApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if !self.style_initialized {
            theme::configure_style(ctx);
            self.style_initialized = true;
        }

        if !self.initial_connect_done && self.first_run_splash.is_none() {
            self.initial_connect_done = true;
            self.workers.submit_api(ApiTask::CheckConnection {
                client: self.client.clone(),
                manual: false,
            });
        }

        // Intercept native window close if downloads are running
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.downloads.busy() {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.dialog = Some(DialogState::Confirm {
                    title: "Downloads running".to_string(),
                    message: "Downloads are still running. Quit anyway?".to_string(),
                    yes_label: "Yes".to_string(),
                    no_label: "No".to_string(),
                    action: ConfirmAction::QuitWhileDownloading,
                });
            } else {
                self.prepare_shutdown(ctx);
                self.allow_close = true;
            }
        }

        if let Some(exp) = self.status_expires {
            if Instant::now() >= exp {
                self.status_msg = "Ready".to_string();
                self.status_expires = None;
            }
        }

        self.drain_events(ctx);

        let mut actions = Vec::new();
        self.handle_global_shortcuts(ctx, &mut actions);

        let bank_post_ids: HashSet<u64> = self.bank_posts.iter().map(|p| p.id).collect();
        let bank_pool_ids: HashSet<u64> = self.bank_pools.iter().map(|p| p.id).collect();
        let bank_artist_set: HashSet<String> = self.bank_artists.iter().cloned().collect();

        let has_post_fn = |id: u64| bank_post_ids.contains(&id);
        let has_pool_fn = |id: u64| bank_pool_ids.contains(&id);
        let has_artist_fn = |name: &str| bank_artist_set.contains(&SpankBank::norm_artist(name));

        if let Some(mut splash) = self.first_run_splash.take() {
            let _ = show_first_run_splash(ctx, &mut splash, self.app_logo.as_ref(), &mut actions);
            self.first_run_splash = Some(splash);
        } else if let Some(mut viewer) = self.viewer.take() {
            show_viewer(
                ctx,
                &mut viewer,
                &self.tag_query,
                &self.preview_textures,
                &self.thumb_textures,
                &has_post_fn,
                &has_pool_fn,
                &has_artist_fn,
                &mut actions,
            );
            let closing = actions.iter().any(|a| matches!(a, UiAction::CloseViewer));
            if !closing {
                self.viewer = Some(viewer);
            }
        } else {
            // Top menu bar
            egui::TopBottomPanel::top("main_menu_bar").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Settings…  (Ctrl+,)").clicked() {
                            actions.push(UiAction::OpenSettings);
                            ui.close_menu();
                        }
                        if ui.button("API Key Setup…").clicked() {
                            actions.push(UiAction::OpenFirstRunSplash);
                            ui.close_menu();
                        }
                        if ui.button("Test Connection").clicked() {
                            actions.push(UiAction::TestConnectionManual);
                            ui.close_menu();
                        }
                        if ui.button("Open download folder").clicked() {
                            actions.push(UiAction::OpenDownloadDir);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Quit  (Ctrl+Q)").clicked() {
                            actions.push(UiAction::RequestQuit);
                            ui.close_menu();
                        }
                    });
                    ui.menu_button("View", |ui| {
                        if ui
                            .checkbox(&mut self.show_inspector, "Tag Inspector")
                            .clicked()
                        {
                            ui.close_menu();
                        }
                    });
                });
            });

            // Bottom status bar
            egui::TopBottomPanel::bottom("main_status_bar")
                .frame(
                    egui::Frame::NONE
                        .fill(theme::BG_SURFACE)
                        .inner_margin(egui::Margin::symmetric(10, 5)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&self.status_msg).size(12.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(&self.auth_status)
                                    .size(12.0)
                                    .color(theme::TEXT_MUTED),
                            );
                        });
                    });
                });

            // Left panel (Search / Pools / Favorites / Spank Bank)
            let left_resp = egui::SidePanel::left("left_sources_panel")
                .default_width(self.settings.left_panel_width.unwrap_or(360.0))
                .min_width(350.0)
                .resizable(true)
                .show(ctx, |ui| {
                    show_left_panel(
                        ui,
                        &mut self.left_tab,
                        &mut self.bank_subtab,
                        &mut self.tag_query,
                        &mut self.autocomplete,
                        &mut self.settings,
                        &mut self.selected_saved_idx,
                        &mut self.pool_query,
                        &self.pool_results,
                        &mut self.selected_pool_idx,
                        &mut self.fav_user,
                        &self.bank_pools,
                        &mut self.bank_pools_filter,
                        &mut self.selected_bank_pool_ids,
                        &mut self.last_bank_pool_id,
                        &self.bank_posts,
                        &mut self.bank_posts_filter,
                        &mut self.selected_bank_post_ids,
                        &self.bank_artists,
                        &mut self.bank_artists_filter,
                        &mut self.bank_artist_add,
                        &mut self.selected_bank_artists,
                        &mut self.last_bank_artist,
                        &has_pool_fn,
                        &mut self.focus_search_requested,
                        &mut actions,
                    );
                });
            let left_w = left_resp.response.rect.width();
            if left_w > 100.0 {
                self.settings.left_panel_width = Some(left_w);
            }

            // Right Tag Inspector
            if self.show_inspector {
                let active_post = self.current_tab().active_post().cloned();
                let active_thumb = active_post
                    .as_ref()
                    .and_then(|p| self.thumb_textures.get(&p.id));
                let in_bank = active_post
                    .as_ref()
                    .map(|p| has_post_fn(p.id))
                    .unwrap_or(false);
                let pool_in_bank = active_post
                    .as_ref()
                    .map(|p| p.pools.iter().any(|&pid| has_pool_fn(pid)))
                    .unwrap_or(false);

                let right_resp = egui::SidePanel::right("right_inspector_panel")
                    .default_width(self.settings.right_panel_width.unwrap_or(280.0))
                    .min_width(260.0)
                    .resizable(true)
                    .show(ctx, |ui| {
                        show_inspector(
                            ui,
                            active_post.as_ref(),
                            active_thumb,
                            &self.tag_query,
                            &mut self.inspector_filter,
                            in_bank,
                            pool_in_bank,
                            &has_artist_fn,
                            &mut actions,
                        );
                    });
                let right_w = right_resp.response.rect.width();
                if right_w > 100.0 {
                    self.settings.right_panel_width = Some(right_w);
                }
            }

            // Center: Tabs + Grid + Download panel
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(theme::BG_WINDOW))
                .show(ctx, |ui| {
                    let selected_cnt = self.current_tab().selected_ids.len();
                    let logs_vec: Vec<String> = self.logs.iter().cloned().collect();
                    let suppress_hover = self.settings_modal.is_some()
                        || self.dialog.is_some()
                        || self.autocomplete.popup_open;

                    egui::TopBottomPanel::top("results_tab_bar_panel")
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(6, 4)))
                        .show_inside(ui, |ui| {
                            show_results_tab_bar(ui, &self.tabs, self.active_tab, &mut actions);
                        });

                    egui::TopBottomPanel::bottom("download_bottom_panel")
                        .resizable(false)
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(6, 4)))
                        .show_inside(ui, |ui| {
                            show_download_panel(
                                ui,
                                &mut self.settings,
                                selected_cnt,
                                self.dl_done,
                                self.dl_total,
                                &logs_vec,
                                &mut actions,
                            );
                        });

                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(6, 2)))
                        .show_inside(ui, |ui| {
                            let source_in_bank = self.source_in_bank(&self.current_tab().source);
                            let history_path_fn = |id: u64| self.history.path_for(id);
                            let active_idx = self.active_tab.min(self.tabs.len() - 1);
                            let tab = &mut self.tabs[active_idx];
                            show_tab_contents(
                                ui,
                                tab,
                                &mut self.pending_scroll_restore,
                                source_in_bank,
                                &self.thumb_textures,
                                &self.preview_textures,
                                &has_post_fn,
                                &has_pool_fn,
                                &has_artist_fn,
                                &history_path_fn,
                                suppress_hover,
                                &mut actions,
                            );
                        });
                });
        }

        if self.first_run_splash.is_none() {
            if let Some(ref mut modal) = self.settings_modal {
                show_settings_modal(ctx, modal, &mut actions);
            }
        }

        self.show_dialog(ctx, &mut actions);
        self.handle_actions(ctx, actions);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.stop_fetch.store(true, Ordering::Relaxed);
        self.workers.stop();
        self.downloads.cancel();
        self.settings.save();
    }
}
