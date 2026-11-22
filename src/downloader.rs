//! Queue-based concurrent downloader with MD5 verification and pool reading-order support.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{mpsc, Arc, Condvar, Mutex};

use md5::{Digest, Md5};

use crate::api::{E621Client, Pool, Post};
use crate::storage::{build_target, DownloadJob, History, ImageCache};
use crate::ui::state::Source;
use crate::workers::{send_event, shared_http_client, BgEvent, DownloadStatus};

struct DlState {
    client: Option<E621Client>,
    queue: VecDeque<(u64, DownloadJob)>,
    gen: u64,
    done: usize,
    total: usize,
    active: usize,
    worker_count: usize,
    pool_cache: HashMap<u64, Option<Pool>>,
    inflight: HashSet<u64>,
}

#[derive(Clone)]
pub struct DownloadManager {
    state: Arc<(Mutex<DlState>, Condvar)>,
    history: History,
    image_cache: Option<ImageCache>,
    ev_tx: mpsc::Sender<BgEvent>,
    egui_ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl DownloadManager {
    pub fn new(
        history: History,
        image_cache: Option<ImageCache>,
        ev_tx: mpsc::Sender<BgEvent>,
        egui_ctx: Arc<Mutex<Option<egui::Context>>>,
    ) -> Self {
        Self {
            state: Arc::new((
                Mutex::new(DlState {
                    client: None,
                    queue: VecDeque::new(),
                    gen: 0,
                    done: 0,
                    total: 0,
                    active: 0,
                    worker_count: 0,
                    pool_cache: HashMap::new(),
                    inflight: HashSet::new(),
                }),
                Condvar::new(),
            )),
            history,
            image_cache,
            ev_tx,
            egui_ctx,
        }
    }

    pub fn configure(&self, client: E621Client, concurrency: usize) {
        let target_workers = concurrency.max(1);
        let to_spawn = {
            let (lock, _) = &*self.state;
            let mut g = lock.lock().expect("download lock poisoned");
            g.client = Some(client);
            let current = g.worker_count;
            if current < target_workers {
                g.worker_count = target_workers;
                target_workers - current
            } else {
                0
            }
        };
        for idx in 0..to_spawn {
            let mgr = self.clone();
            std::thread::Builder::new()
                .name(format!("download-{idx}"))
                .spawn(move || mgr.worker_loop())
                .expect("failed to spawn download worker");
        }
    }

    pub fn enqueue(&self, jobs: Vec<DownloadJob>) {
        if jobs.is_empty() {
            return;
        }
        let (done, total) = {
            let (lock, cv) = &*self.state;
            let mut g = lock.lock().expect("download lock poisoned");
            if g.done >= g.total {
                g.done = 0;
                g.total = 0;
            }
            g.total += jobs.len();
            let gen = g.gen;
            for j in jobs {
                g.queue.push_back((gen, j));
            }
            cv.notify_all();
            (g.done, g.total)
        };
        send_event(
            &self.ev_tx,
            &self.egui_ctx,
            BgEvent::DownloadProgress { done, total },
        );
    }

    pub fn cancel(&self) {
        let (done, total) = {
            let (lock, _) = &*self.state;
            let mut g = lock.lock().expect("download lock poisoned");
            g.gen = g.gen.wrapping_add(1);
            g.queue.clear();
            g.total = g.done + g.active;
            (g.done, g.total)
        };
        send_event(
            &self.ev_tx,
            &self.egui_ctx,
            BgEvent::DownloadProgress { done, total },
        );
        if total == done {
            send_event(&self.ev_tx, &self.egui_ctx, BgEvent::DownloadsIdle);
        }
    }

    pub fn busy(&self) -> bool {
        let (lock, _) = &*self.state;
        let g = lock.lock().expect("download lock poisoned");
        g.done < g.total
    }

    fn is_cancelled(&self, gen: u64) -> bool {
        let (lock, _) = &*self.state;
        let g = lock.lock().expect("download lock poisoned");
        g.gen != gen
    }

    fn lookup_pool(&self, pool_id: u64) -> Option<Pool> {
        let client = {
            let (lock, _) = &*self.state;
            let g = lock.lock().expect("download lock poisoned");
            if let Some(cached) = g.pool_cache.get(&pool_id) {
                return cached.clone();
            }
            g.client.clone()
        };
        let pool = client.and_then(|c| c.get_pool(pool_id).ok());
        let (lock, _) = &*self.state;
        let mut g = lock.lock().expect("download lock poisoned");
        g.pool_cache.insert(pool_id, pool.clone());
        pool
    }

    fn worker_loop(&self) {
        loop {
            let (gen, job, duplicate) = {
                let (lock, cv) = &*self.state;
                let mut g = lock.lock().expect("download lock poisoned");
                loop {
                    if let Some((item_gen, job)) = g.queue.pop_front() {
                        if item_gen != g.gen {
                            continue;
                        }
                        g.active += 1;
                        let dup = !g.inflight.insert(job.post.id);
                        break (item_gen, job, dup);
                    }
                    g = cv.wait(g).expect("download condvar wait failed");
                }
            };

            let (status, detail) = if duplicate {
                (DownloadStatus::Skipped, "already downloading".to_string())
            } else {
                self.download_job(gen, &job)
            };

            let (done, total) = {
                let (lock, _) = &*self.state;
                let mut g = lock.lock().expect("download lock poisoned");
                if !duplicate {
                    g.inflight.remove(&job.post.id);
                }
                g.active = g.active.saturating_sub(1);
                g.done += 1;
                (g.done, g.total)
            };

            send_event(
                &self.ev_tx,
                &self.egui_ctx,
                BgEvent::DownloadItemFinished {
                    post_id: job.post.id,
                    status,
                    detail,
                },
            );
            send_event(
                &self.ev_tx,
                &self.egui_ctx,
                BgEvent::DownloadProgress { done, total },
            );
            if done >= total {
                send_event(&self.ev_tx, &self.egui_ctx, BgEvent::DownloadsIdle);
            }
        }
    }

    fn download_job(&self, gen: u64, job: &DownloadJob) -> (DownloadStatus, String) {
        let post = &job.post;
        let Some(ref file_url) = post.file_url else {
            return (
                DownloadStatus::Error,
                "no file URL (deleted, or hidden from anonymous users: add your API key in Settings)".to_string(),
            );
        };

        let target = build_target(job, |pid| self.lookup_pool(pid));
        if job.skip_downloaded
            && target.exists()
            && fs::metadata(&target).map(|m| m.len() > 0).unwrap_or(false)
        {
            self.history.add(post, &target);
            return (
                DownloadStatus::Skipped,
                format!("exists: {}", target.display()),
            );
        }

        // Reuse cached image only if it is the original file (no resized sample) and matches MD5
        if let Some(ref ic) = self.image_cache {
            if !post.is_non_image_media()
                && (post.sample_url.is_none() || post.sample_url.as_deref() == Some(file_url.as_str()))
            {
                let cpath = ic.preview_path(post.id, &post.ext);
                if cpath.exists()
                    && fs::metadata(&cpath).map(|m| m.len() > 0).unwrap_or(false)
                    && md5_matches_file(&cpath, &post.md5)
                {
                    if let Some(parent) = target.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    if fs::copy(&cpath, &target).is_ok() {
                        if job.write_tags {
                            let _ = write_tags_file(&target, post);
                        }
                        self.history.add(post, &target);
                        return (DownloadStatus::Ok, target.display().to_string());
                    }
                }
            }
        }

        if let Some(parent) = target.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return (DownloadStatus::Error, e.to_string());
            }
        }
        let part = match target.file_name().and_then(|n| n.to_str()) {
            Some(name) => target.with_file_name(format!("{name}.part")),
            None => target.with_extension("part"),
        };

        let mut last_err = String::new();
        let mut success = false;
        for _attempt in 0..3 {
            if self.is_cancelled(gen) {
                let _ = fs::remove_file(&part);
                return (DownloadStatus::Cancelled, String::new());
            }

            match self.stream_to_part(gen, file_url, &part, &post.md5) {
                Ok(true) => {
                    if let Err(e) = fs::rename(&part, &target) {
                        last_err = e.to_string();
                        let _ = fs::remove_file(&part);
                    } else {
                        success = true;
                        break;
                    }
                }
                Ok(false) => {
                    let _ = fs::remove_file(&part);
                    return (DownloadStatus::Cancelled, String::new());
                }
                Err(e) => {
                    last_err = e;
                    let _ = fs::remove_file(&part);
                }
            }
        }

        if !success {
            return (DownloadStatus::Error, last_err);
        }

        if job.write_tags {
            let _ = write_tags_file(&target, post);
        }
        self.history.add(post, &target);
        (DownloadStatus::Ok, target.display().to_string())
    }

    fn stream_to_part(
        &self,
        gen: u64,
        url: &str,
        part: &Path,
        expected_md5: &str,
    ) -> Result<bool, String> {
        let mut resp = shared_http_client()
            .get(url)
            .send()
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status().as_u16()));
        }
        let mut fh = File::create(part).map_err(|e| e.to_string())?;
        let mut hasher = Md5::new();
        let mut buf = [0u8; 65536];
        loop {
            if self.is_cancelled(gen) {
                return Ok(false);
            }
            let n = resp.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            fh.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            hasher.update(&buf[..n]);
        }
        fh.flush().map_err(|e| e.to_string())?;
        if !expected_md5.is_empty() {
            let digest = format!("{:x}", hasher.finalize());
            if digest != expected_md5 {
                return Err("MD5 mismatch".to_string());
            }
        }
        Ok(true)
    }
}

fn md5_matches_file(path: &Path, expected_md5: &str) -> bool {
    if expected_md5.is_empty() {
        return true;
    }
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let digest = format!("{:x}", Md5::digest(&bytes));
    digest == expected_md5
}

pub fn format_tags_txt(post: &Post) -> String {
    let order = [
        "artist",
        "copyright",
        "character",
        "species",
        "general",
        "lore",
        "meta",
    ];
    let mut tags: Vec<String> = Vec::new();
    for cat in order {
        if let Some(list) = post.tags.get(cat) {
            for t in list {
                tags.push(t.replace('_', " "));
            }
        }
    }
    format!("{}\n", tags.join(", "))
}

pub fn write_tags_file(target: &Path, post: &Post) -> std::io::Result<()> {
    let txt_path = target.with_extension("txt");
    fs::write(txt_path, format_tags_txt(post))
}

pub fn make_download_jobs(
    posts: &[Post],
    src: &Source,
    base_dir: &str,
    mode: &str,
    write_tags: bool,
    skip_downloaded: bool,
    saved_pools: &[Pool],
) -> Vec<DownloadJob> {
    let idx_map: HashMap<u64, usize> = src
        .pool
        .as_ref()
        .map(|pool| {
            pool.post_ids
                .iter()
                .enumerate()
                .map(|(i, &pid)| (pid, i + 1))
                .collect()
        })
        .unwrap_or_default();

    let saved_pool_map: HashMap<u64, &Pool> = if matches!(mode, "pool" | "search") && src.pool.is_none() {
        saved_pools.iter().map(|p| (p.id, p)).collect()
    } else {
        HashMap::new()
    };

    posts
        .iter()
        .map(|p| {
            let mut j = DownloadJob::new(p.clone(), base_dir, mode);
            j.source = src.label.clone();
            j.write_tags = write_tags;
            j.skip_downloaded = skip_downloaded;

            if let Some(ref pool) = src.pool {
                j.pool_id = Some(pool.id);
                j.pool_name = Some(pool.display_name());
                j.pool_index = idx_map.get(&p.id).copied();
            } else if !saved_pool_map.is_empty() && !p.pools.is_empty() {
                for &pid in &p.pools {
                    if let Some(sp) = saved_pool_map.get(&pid) {
                        j.pool_id = Some(sp.id);
                        j.pool_name = Some(sp.display_name());
                        j.pool_index = sp
                            .post_ids
                            .iter()
                            .position(|&id| id == p.id)
                            .map(|i| i + 1);
                        break;
                    }
                }
            }
            j
        })
        .collect()
}
