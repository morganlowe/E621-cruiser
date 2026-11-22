//! Background worker pools for API calls, tag autocomplete, and image/thumbnail fetching.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

use reqwest::blocking::Client;

use crate::api::{user_agent, E621Client, Pool, Post, Tag};
use crate::storage::ImageCache;
use crate::ui::state::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolMetaAction {
    Open,
    OpenNewTab,
    ToggleBank,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    Ok,
    Skipped,
    Error,
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum BgEvent {
    ConnectionChecked {
        ok: bool,
        msg: String,
        manual: bool,
    },
    SettingsTestResult {
        ok: bool,
        msg: String,
    },
    AutocompleteResult {
        token: String,
        tags: Vec<Tag>,
    },
    PostsPageLoaded {
        tab_id: u64,
        gen: u64,
        posts: Vec<Post>,
    },
    PoolPostsLoaded {
        tab_id: u64,
        gen: u64,
        posts: Vec<Post>,
    },
    PoolMetadataLoaded {
        pool: Pool,
        action: PoolMetaAction,
    },
    PoolsSearchResult {
        pools: Vec<Pool>,
    },
    SlideshowPostsLoaded {
        posts: Vec<Post>,
        shuffle: bool,
        title: String,
    },
    DownloadAllCollected {
        tab_id: u64,
        gen: u64,
        source: Source,
        shown: Vec<Post>,
        more: Vec<Post>,
    },
    FetchAllStatus {
        msg: String,
    },
    ApiError {
        tab_id: Option<u64>,
        gen: Option<u64>,
        err: String,
    },
    FavoriteToggled {
        post_id: u64,
        is_favorited: bool,
    },
    FavoriteError(String),
    UpvoteToggled {
        post_id: u64,
        own_vote: i32,
    },
    UpvoteError(String),
    ThumbLoaded {
        tab_id: u64,
        gen: u64,
        post_id: u64,
        image: Arc<egui::ColorImage>,
    },
    PreviewLoaded {
        post_id: u64,
        image: Arc<egui::ColorImage>,
        for_viewer: bool,
    },
    PreviewError {
        post_id: u64,
        err: String,
    },
    DownloadProgress {
        done: usize,
        total: usize,
    },
    DownloadItemFinished {
        post_id: u64,
        status: DownloadStatus,
        detail: String,
    },
    DownloadsIdle,
    FolderPicked(Option<PathBuf>),
}

static SHARED_HTTP: OnceLock<Client> = OnceLock::new();
static TMP_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn shared_http_client() -> &'static Client {
    SHARED_HTTP.get_or_init(|| {
        Client::builder()
            .user_agent(user_agent(""))
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| Client::new())
    })
}

pub fn send_event(
    tx: &mpsc::Sender<BgEvent>,
    ctx: &Arc<Mutex<Option<egui::Context>>>,
    ev: BgEvent,
) {
    let _ = tx.send(ev);
    if let Ok(guard) = ctx.lock() {
        if let Some(ref c) = *guard {
            c.request_repaint();
        }
    }
}

pub fn is_non_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "webm" | "mp4" | "swf"))
        .unwrap_or(false)
}

pub fn decode_color_image(bytes: &[u8]) -> Result<egui::ColorImage, String> {
    let dyn_img = image::load_from_memory(bytes).map_err(|e| format!("Decode error: {e}"))?;
    let rgba = dyn_img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        rgba.as_raw(),
    ))
}

/// Fetch an image checking `local_path`, then `cache_path`, then calling `fetcher(url)`.
/// Writes decoded images atomically into `cache_path` when downloaded over the network.
pub fn fetch_image_with_fetcher<F>(
    url: &str,
    cache_path: Option<&Path>,
    local_path: Option<&Path>,
    mut fetcher: F,
) -> Result<egui::ColorImage, String>
where
    F: FnMut(&str) -> Result<Vec<u8>, String>,
{
    if let Some(lp) = local_path {
        if !is_non_image_path(lp) && lp.exists() {
            if let Ok(meta) = fs::metadata(lp) {
                if meta.len() > 0 {
                    if let Ok(bytes) = fs::read(lp) {
                        if let Ok(img) = decode_color_image(&bytes) {
                            return Ok(img);
                        }
                    }
                }
            }
        }
    }

    if let Some(cp) = cache_path {
        if !is_non_image_path(cp) && cp.exists() {
            if let Ok(meta) = fs::metadata(cp) {
                if meta.len() > 0 {
                    if let Ok(bytes) = fs::read(cp) {
                        if let Ok(img) = decode_color_image(&bytes) {
                            return Ok(img);
                        }
                    }
                }
            }
        }
    }

    if url.trim().is_empty() {
        return Err("No image URL available".to_string());
    }

    let bytes = fetcher(url)?;
    let img = decode_color_image(&bytes)?;

    if let Some(cp) = cache_path {
        if !is_non_image_path(cp) {
            if let Some(parent) = cp.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Some(fname) = cp.file_name().and_then(|n| n.to_str()) {
                let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
                let tmp = cp.with_file_name(format!("{fname}.{}.{seq}.tmp", std::process::id()));
                if fs::write(&tmp, &bytes).is_ok() {
                    let _ = fs::rename(&tmp, cp);
                }
            }
        }
    }

    Ok(img)
}

pub fn fetch_image(
    url: &str,
    timeout_secs: u64,
    cache_path: Option<&Path>,
    local_path: Option<&Path>,
) -> Result<egui::ColorImage, String> {
    fetch_image_with_fetcher(url, cache_path, local_path, |target_url| {
        let resp = shared_http_client()
            .get(target_url)
            .timeout(Duration::from_secs(timeout_secs))
            .send()
            .map_err(|e| format!("Network error: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status().as_u16()));
        }
        let bytes = resp
            .bytes()
            .map_err(|e| format!("Read error: {e}"))?
            .to_vec();
        Ok(bytes)
    })
}

pub enum ApiTask {
    CheckConnection {
        client: E621Client,
        manual: bool,
    },
    TestSettingsConnection {
        client: E621Client,
    },
    SearchPosts {
        client: E621Client,
        tab_id: u64,
        gen: u64,
        query: String,
        page: u32,
    },
    LoadPoolPosts {
        client: E621Client,
        tab_id: u64,
        gen: u64,
        pool: Pool,
    },
    FetchPoolMetadata {
        client: E621Client,
        pool_id: u64,
        action: PoolMetaAction,
    },
    SearchPools {
        client: E621Client,
        query: String,
    },
    LoadSlideshowPosts {
        client: E621Client,
        query: String,
        pool: Option<Pool>,
        shuffle: bool,
        title: String,
    },
    CollectAllPosts {
        client: E621Client,
        tab_id: u64,
        gen: u64,
        source: Source,
        shown: Vec<Post>,
        start_page: u32,
        stop_flag: Arc<AtomicBool>,
    },
    ToggleFavorite {
        client: E621Client,
        post_id: u64,
        make_favorite: bool,
    },
    ToggleUpvote {
        client: E621Client,
        post_id: u64,
        make_upvote: bool,
    },
}

struct TagTask {
    client: E621Client,
    token: String,
    seq: u64,
}

enum ImgTask {
    Thumb {
        tab_id: u64,
        gen: u64,
        queue_gen: u64,
        post_id: u64,
        url: String,
        cache_path: PathBuf,
        image_cache: ImageCache,
    },
    Preview {
        post_id: u64,
        url: String,
        cache_path: PathBuf,
        local_path: Option<PathBuf>,
        for_viewer: bool,
        prefetch_only: bool,
        image_cache: ImageCache,
    },
}

struct ImgQueueInner {
    tasks: VecDeque<ImgTask>,
    queue_gen: u64,
    stopped: bool,
}

pub struct WorkerPool {
    api_tx: mpsc::Sender<ApiTask>,
    tag_tx: mpsc::Sender<TagTask>,
    tag_seq: Arc<AtomicU64>,
    img_queue: Arc<(Mutex<ImgQueueInner>, Condvar)>,
    ev_tx: mpsc::Sender<BgEvent>,
    egui_ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl WorkerPool {
    pub fn new(ev_tx: mpsc::Sender<BgEvent>) -> Self {
        let egui_ctx: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));
        let (api_tx, api_rx) = mpsc::channel::<ApiTask>();
        let (tag_tx, tag_rx) = mpsc::channel::<TagTask>();
        let tag_seq = Arc::new(AtomicU64::new(0));
        let img_queue = Arc::new((
            Mutex::new(ImgQueueInner {
                tasks: VecDeque::new(),
                queue_gen: 0,
                stopped: false,
            }),
            Condvar::new(),
        ));

        // API worker (sequential inside client rate limiter)
        {
            let tx = ev_tx.clone();
            let ctx = Arc::clone(&egui_ctx);
            std::thread::Builder::new()
                .name("e621-api".to_string())
                .spawn(move || {
                    while let Ok(task) = api_rx.recv() {
                        run_api_task(task, &tx, &ctx);
                    }
                })
                .expect("failed to spawn api worker");
        }

        // Tag autocomplete worker
        {
            let tx = ev_tx.clone();
            let ctx = Arc::clone(&egui_ctx);
            let latest_seq = Arc::clone(&tag_seq);
            std::thread::Builder::new()
                .name("e621-tags".to_string())
                .spawn(move || {
                    while let Ok(task) = tag_rx.recv() {
                        if task.seq != latest_seq.load(Ordering::Relaxed) {
                            continue;
                        }
                        let tags = task.client.autocomplete(&task.token);
                        if task.seq == latest_seq.load(Ordering::Relaxed) {
                            send_event(
                                &tx,
                                &ctx,
                                BgEvent::AutocompleteResult {
                                    token: task.token,
                                    tags,
                                },
                            );
                        }
                    }
                })
                .expect("failed to spawn tag worker");
        }

        // 6 thumbnail / preview CDN workers
        for i in 0..6 {
            let q = Arc::clone(&img_queue);
            let tx = ev_tx.clone();
            let ctx = Arc::clone(&egui_ctx);
            std::thread::Builder::new()
                .name(format!("e621-img-{i}"))
                .spawn(move || loop {
                    let task = {
                        let (lock, cv) = &*q;
                        let mut g = lock.lock().expect("img queue lock poisoned");
                        loop {
                            if g.stopped {
                                return;
                            }
                            if let Some(t) = g.tasks.pop_front() {
                                match &t {
                                    ImgTask::Thumb { queue_gen, .. }
                                        if *queue_gen != g.queue_gen =>
                                    {
                                        continue;
                                    }
                                    _ => break t,
                                }
                            }
                            g = cv.wait(g).expect("img queue condvar wait failed");
                        }
                    };
                    run_img_task(task, &q.0, &tx, &ctx);
                })
                .expect("failed to spawn image worker");
        }

        Self {
            api_tx,
            tag_tx,
            tag_seq,
            img_queue,
            ev_tx,
            egui_ctx,
        }
    }

    pub fn set_egui_ctx(&self, ctx: egui::Context) {
        let mut g = self.egui_ctx.lock().expect("ctx lock poisoned");
        *g = Some(ctx);
    }

    pub fn event_sender(&self) -> mpsc::Sender<BgEvent> {
        self.ev_tx.clone()
    }

    pub fn ctx_handle(&self) -> Arc<Mutex<Option<egui::Context>>> {
        Arc::clone(&self.egui_ctx)
    }

    pub fn submit_api(&self, task: ApiTask) {
        let _ = self.api_tx.send(task);
    }

    pub fn submit_autocomplete(&self, client: E621Client, token: String) {
        let seq = self.tag_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.tag_tx.send(TagTask { client, token, seq });
    }

    pub fn clear_pending_thumbs(&self) {
        let (lock, _) = &*self.img_queue;
        let mut g = lock.lock().expect("img queue lock poisoned");
        g.queue_gen = g.queue_gen.wrapping_add(1);
        g.tasks
            .retain(|t| matches!(t, ImgTask::Preview { .. }));
    }

    pub fn submit_thumb(
        &self,
        tab_id: u64,
        gen: u64,
        post_id: u64,
        url: String,
        cache_path: PathBuf,
        image_cache: ImageCache,
    ) {
        let (lock, cv) = &*self.img_queue;
        let mut g = lock.lock().expect("img queue lock poisoned");
        let queue_gen = g.queue_gen;
        g.tasks.push_back(ImgTask::Thumb {
            tab_id,
            gen,
            queue_gen,
            post_id,
            url,
            cache_path,
            image_cache,
        });
        cv.notify_one();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit_preview(
        &self,
        post_id: u64,
        url: String,
        cache_path: PathBuf,
        local_path: Option<PathBuf>,
        for_viewer: bool,
        prefetch_only: bool,
        image_cache: ImageCache,
    ) {
        let (lock, cv) = &*self.img_queue;
        let mut g = lock.lock().expect("img queue lock poisoned");
        let task = ImgTask::Preview {
            post_id,
            url,
            cache_path,
            local_path,
            for_viewer,
            prefetch_only,
            image_cache,
        };
        if prefetch_only {
            g.tasks.push_back(task);
        } else {
            g.tasks.push_front(task);
        }
        cv.notify_one();
    }

    pub fn pick_folder_async(&self, start_dir: Option<PathBuf>) {
        let tx = self.ev_tx.clone();
        let ctx = Arc::clone(&self.egui_ctx);
        std::thread::Builder::new()
            .name("e621-folder-picker".to_string())
            .spawn(move || {
                let picked = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok()
                    .and_then(|rt| {
                        rt.block_on(async move {
                            let mut dlg = rfd::AsyncFileDialog::new().set_title("Download folder");
                            if let Some(dir) = start_dir {
                                if dir.exists() {
                                    dlg = dlg.set_directory(dir);
                                }
                            }
                            dlg.pick_folder().await.map(|h| h.path().to_path_buf())
                        })
                    });
                send_event(&tx, &ctx, BgEvent::FolderPicked(picked));
            })
            .ok();
    }

    pub fn stop(&self) {
        let (lock, cv) = &*self.img_queue;
        let mut g = lock.lock().expect("img queue lock poisoned");
        g.stopped = true;
        g.tasks.clear();
        cv.notify_all();
    }
}

fn run_api_task(
    task: ApiTask,
    tx: &mpsc::Sender<BgEvent>,
    ctx: &Arc<Mutex<Option<egui::Context>>>,
) {
    match task {
        ApiTask::CheckConnection { client, manual } => {
            let (ok, msg) = client.check_connection();
            send_event(tx, ctx, BgEvent::ConnectionChecked { ok, msg, manual });
        }
        ApiTask::TestSettingsConnection { client } => {
            let (ok, msg) = client.check_connection();
            send_event(tx, ctx, BgEvent::SettingsTestResult { ok, msg });
        }
        ApiTask::SearchPosts {
            client,
            tab_id,
            gen,
            query,
            page,
        } => match client.search_posts(&query, page, crate::api::POSTS_PER_PAGE) {
            Ok(posts) => {
                send_event(
                    tx,
                    ctx,
                    BgEvent::PostsPageLoaded { tab_id, gen, posts },
                );
            }
            Err(e) => {
                send_event(
                    tx,
                    ctx,
                    BgEvent::ApiError {
                        tab_id: Some(tab_id),
                        gen: Some(gen),
                        err: e.0,
                    },
                );
            }
        },
        ApiTask::LoadPoolPosts {
            client,
            tab_id,
            gen,
            pool,
        } => match client.get_pool_posts(&pool) {
            Ok(posts) => {
                send_event(
                    tx,
                    ctx,
                    BgEvent::PoolPostsLoaded { tab_id, gen, posts },
                );
            }
            Err(e) => {
                send_event(
                    tx,
                    ctx,
                    BgEvent::ApiError {
                        tab_id: Some(tab_id),
                        gen: Some(gen),
                        err: e.0,
                    },
                );
            }
        },
        ApiTask::FetchPoolMetadata {
            client,
            pool_id,
            action,
        } => match client.get_pool(pool_id) {
            Ok(pool) => {
                send_event(tx, ctx, BgEvent::PoolMetadataLoaded { pool, action });
            }
            Err(e) => {
                send_event(
                    tx,
                    ctx,
                    BgEvent::ApiError {
                        tab_id: None,
                        gen: None,
                        err: e.0,
                    },
                );
            }
        },
        ApiTask::SearchPools { client, query } => match client.search_pools(&query, 1) {
            Ok(pools) => {
                send_event(tx, ctx, BgEvent::PoolsSearchResult { pools });
            }
            Err(e) => {
                send_event(
                    tx,
                    ctx,
                    BgEvent::ApiError {
                        tab_id: None,
                        gen: None,
                        err: e.0,
                    },
                );
            }
        },
        ApiTask::LoadSlideshowPosts {
            client,
            query,
            pool,
            shuffle,
            title,
        } => {
            let res = match pool {
                Some(p) => client.get_pool_posts(&p),
                None => client.search_posts(&query, 1, crate::api::POSTS_PER_PAGE),
            };
            match res {
                Ok(posts) => {
                    send_event(
                        tx,
                        ctx,
                        BgEvent::SlideshowPostsLoaded {
                            posts,
                            shuffle,
                            title,
                        },
                    );
                }
                Err(e) => {
                    send_event(
                        tx,
                        ctx,
                        BgEvent::ApiError {
                            tab_id: None,
                            gen: None,
                            err: e.0,
                        },
                    );
                }
            }
        }
        ApiTask::CollectAllPosts {
            client,
            tab_id,
            gen,
            source,
            shown,
            start_page,
            stop_flag,
        } => {
            let shown_len = shown.len();
            let res = client.fetch_all_posts(
                &source.query,
                start_page,
                |pg, n| {
                    send_event(
                        tx,
                        ctx,
                        BgEvent::FetchAllStatus {
                            msg: format!("Collecting results… page {pg}, {} posts", shown_len + n),
                        },
                    );
                },
                || stop_flag.load(Ordering::Relaxed),
            );
            match res {
                Ok(more) => {
                    send_event(
                        tx,
                        ctx,
                        BgEvent::DownloadAllCollected {
                            tab_id,
                            gen,
                            source,
                            shown,
                            more,
                        },
                    );
                }
                Err(e) => {
                    send_event(
                        tx,
                        ctx,
                        BgEvent::ApiError {
                            tab_id: Some(tab_id),
                            gen: Some(gen),
                            err: e.0,
                        },
                    );
                }
            }
        }
        ApiTask::ToggleFavorite {
            client,
            post_id,
            make_favorite,
        } => {
            let res = if make_favorite {
                client.add_favorite(post_id)
            } else {
                client.remove_favorite(post_id)
            };
            match res {
                Ok(()) => {
                    send_event(
                        tx,
                        ctx,
                        BgEvent::FavoriteToggled {
                            post_id,
                            is_favorited: make_favorite,
                        },
                    );
                }
                Err(e) => {
                    send_event(tx, ctx, BgEvent::FavoriteError(e.0));
                }
            }
        }
        ApiTask::ToggleUpvote {
            client,
            post_id,
            make_upvote,
        } => {
            let res = if make_upvote {
                client.vote_post(post_id, 1)
            } else {
                client.remove_vote(post_id)
            };
            match res {
                Ok(()) => {
                    send_event(
                        tx,
                        ctx,
                        BgEvent::UpvoteToggled {
                            post_id,
                            own_vote: if make_upvote { 1 } else { 0 },
                        },
                    );
                }
                Err(e) => {
                    send_event(tx, ctx, BgEvent::UpvoteError(e.0));
                }
            }
        }
    }
}

fn run_img_task(
    task: ImgTask,
    queue_lock: &Mutex<ImgQueueInner>,
    tx: &mpsc::Sender<BgEvent>,
    ctx: &Arc<Mutex<Option<egui::Context>>>,
) {
    match task {
        ImgTask::Thumb {
            tab_id,
            gen,
            queue_gen,
            post_id,
            url,
            cache_path,
            image_cache,
        } => {
            let cur_qgen = queue_lock
                .lock()
                .map(|g| g.queue_gen)
                .unwrap_or(queue_gen);
            if cur_qgen != queue_gen {
                return;
            }
            if let Ok(img) = fetch_image(&url, 15, Some(&cache_path), None) {
                let arc = Arc::new(img);
                image_cache.set_memory_thumb(post_id, Arc::clone(&arc));
                send_event(
                    tx,
                    ctx,
                    BgEvent::ThumbLoaded {
                        tab_id,
                        gen,
                        post_id,
                        image: arc,
                    },
                );
            }
        }
        ImgTask::Preview {
            post_id,
            url,
            cache_path,
            local_path,
            for_viewer,
            prefetch_only,
            image_cache,
        } => {
            match fetch_image(&url, 15, Some(&cache_path), local_path.as_deref()) {
                Ok(img) => {
                    let arc = Arc::new(img);
                    image_cache.set_memory_preview(post_id, Arc::clone(&arc));
                    if !prefetch_only {
                        send_event(
                            tx,
                            ctx,
                            BgEvent::PreviewLoaded {
                                post_id,
                                image: arc,
                                for_viewer,
                            },
                        );
                    }
                }
                Err(err) => {
                    if !prefetch_only && for_viewer {
                        send_event(tx, ctx, BgEvent::PreviewError { post_id, err });
                    }
                }
            }
        }
    }
}
