"""Queue-based concurrent downloader."""
from __future__ import annotations

import hashlib
import os
import queue
import threading
from pathlib import Path

import requests
from PySide6.QtCore import QObject, Signal

from .api import ApiError, E621Client, Pool
from .storage import DownloadJob, History, build_target
from .workers import http_session


class DownloadManager(QObject):
    progress = Signal(int, int)             # done, total
    item_finished = Signal(int, str, str)   # post_id, status, detail
    idle = Signal()

    def __init__(self, history: History, parent: QObject | None = None):
        super().__init__(parent)
        self.history = history
        self.client: E621Client | None = None
        self._queue: queue.Queue[tuple[int, DownloadJob]] = queue.Queue()
        self._lock = threading.Lock()
        self._gen = 0
        self._done = 0
        self._total = 0
        self._active = 0
        self._workers: list[threading.Thread] = []
        self._pool_cache: dict[int, Pool | None] = {}
        self._inflight: set[int] = set()

    # ------------------------------------------------------------ public API
    def configure(self, client: E621Client, concurrency: int) -> None:
        self.client = client
        while len(self._workers) < max(1, concurrency):
            t = threading.Thread(target=self._worker, daemon=True,
                                 name=f"download-{len(self._workers)}")
            t.start()
            self._workers.append(t)

    def enqueue(self, jobs: list[DownloadJob]) -> None:
        if not jobs:
            return
        with self._lock:
            if self._done >= self._total:
                self._done = self._total = 0
            self._total += len(jobs)
            gen = self._gen
            done, total = self._done, self._total
        for j in jobs:
            self._queue.put((gen, j))
        self.progress.emit(done, total)

    def cancel(self) -> None:
        with self._lock:
            self._gen += 1
            try:
                while True:
                    self._queue.get_nowait()
            except queue.Empty:
                pass
            self._total = self._done + self._active
            done, total = self._done, self._total
        self.progress.emit(done, total)
        if total == done:
            self.idle.emit()

    @property
    def busy(self) -> bool:
        with self._lock:
            return self._done < self._total

    # -------------------------------------------------------------- internal
    def _lookup_pool(self, pool_id: int) -> Pool | None:
        with self._lock:
            if pool_id in self._pool_cache:
                return self._pool_cache[pool_id]
        pool = None
        if self.client is not None:
            try:
                pool = self.client.get_pool(pool_id)
            except ApiError:
                pool = None
        with self._lock:
            self._pool_cache[pool_id] = pool
        return pool

    def _worker(self) -> None:
        while True:
            gen, job = self._queue.get()
            with self._lock:
                if gen != self._gen:
                    continue
                self._active += 1
            with self._lock:
                duplicate = job.post.id in self._inflight
                self._inflight.add(job.post.id)
            try:
                if duplicate:
                    status, detail = "skipped", "already downloading"
                else:
                    status, detail = self._download(gen, job)
            except Exception as e:  # noqa: BLE001
                status, detail = "error", str(e)
            finally:
                if not duplicate:
                    with self._lock:
                        self._inflight.discard(job.post.id)
            with self._lock:
                self._active -= 1
                self._done += 1
                done, total = self._done, self._total
            self.item_finished.emit(job.post.id, status, detail)
            self.progress.emit(done, total)
            if done >= total:
                self.idle.emit()

    def _cancelled(self, gen: int) -> bool:
        return gen != self._gen

    def _download(self, gen: int, job: DownloadJob) -> tuple[str, str]:
        post = job.post
        if not post.file_url:
            return "error", ("no file URL (deleted, or hidden from anonymous users: "
                             "add your API key in Settings)")
        if job.skip_downloaded and self.history.has(post.id):
            return "skipped", "already in download history"

        target = build_target(job, self._lookup_pool)
        if target.exists():
            self.history.add(post, target)
            return "skipped", f"exists: {target}"

        target.parent.mkdir(parents=True, exist_ok=True)
        part = target.with_name(target.name + ".part")
        last_err = ""
        for attempt in range(3):
            if self._cancelled(gen):
                return "cancelled", ""
            md5 = hashlib.md5()
            try:
                with http_session().get(post.file_url, stream=True, timeout=60) as r:
                    r.raise_for_status()
                    with open(part, "wb") as fh:
                        for chunk in r.iter_content(1 << 16):
                            if self._cancelled(gen):
                                break
                            fh.write(chunk)
                            md5.update(chunk)
                if self._cancelled(gen):
                    part.unlink(missing_ok=True)
                    return "cancelled", ""
                if post.md5 and md5.hexdigest() != post.md5:
                    raise OSError("MD5 mismatch")
                os.replace(part, target)
                break
            except (requests.RequestException, OSError) as e:
                last_err = str(e)
                part.unlink(missing_ok=True)
        else:
            return "error", last_err

        if job.write_tags:
            self._write_tags(target, job)
        self.history.add(post, target)
        return "ok", str(target)

    @staticmethod
    def _write_tags(target: Path, job: DownloadJob) -> None:
        p = job.post
        order = ["artist", "copyright", "character", "species", "general", "lore", "meta"]
        tags: list[str] = []
        for cat in order:
            tags.extend(p.tags.get(cat, []))
        target.with_suffix(".txt").write_text(", ".join(t.replace("_", " ") for t in tags) + "\n")
