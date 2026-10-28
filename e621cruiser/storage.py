"""Download history (SQLite) and on-disk path layout."""
from __future__ import annotations

import json
import re
import sqlite3
import threading
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Callable

from .api import Pool, Post

ORGANIZE_MODES: list[tuple[str, str]] = [
    ("search", "By search / pool (where it came from)"),
    ("artist", "By artist"),
    ("pool", "By pool"),
    ("copyright", "By copyright (series / franchise)"),
    ("character", "By character"),
    ("species", "By species"),
    ("rating", "By rating"),
    ("flat", "Flat (everything in one folder)"),
]

RATING_NAMES = {"s": "safe", "q": "questionable", "e": "explicit"}

_BAD_CHARS = re.compile(r'[<>:"/\\|?*\x00-\x1f]')
_WINDOWS_RESERVED = {
    "CON", "PRN", "AUX", "NUL",
    *(f"COM{i}" for i in range(1, 10)),
    *(f"LPT{i}" for i in range(1, 10)),
}


def sanitize(name: str, maxlen: int = 120) -> str:
    name = _BAD_CHARS.sub("_", name).strip(" .")
    clean = name[:maxlen].rstrip(" .") or "_"
    if clean.upper() in _WINDOWS_RESERVED:
        clean = f"_{clean}_"
    return clean


@dataclass
class DownloadJob:
    post: Post
    base_dir: str
    mode: str
    source: str = ""               # search query / favorites label
    pool_id: int | None = None
    pool_name: str | None = None
    pool_index: int | None = None  # 1-based position in pool
    write_tags: bool = False
    skip_downloaded: bool = True


def build_target(job: DownloadJob, pool_lookup: Callable[[int], Pool | None]) -> Path:
    """Work out where a post should be saved for the chosen organize mode."""
    post, base, mode = job.post, Path(job.base_dir).expanduser(), job.mode
    fname = f"{post.id}.{post.ext}"

    def pool_path(pid: int, name: str | None, idx: int | None) -> Path:
        folder = base / "pools" / sanitize(f"{name or 'pool'} ({pid})")
        return folder / (f"{idx:04d}_{fname}" if idx else fname)

    if mode in ("search", "pool") and job.pool_id is not None:
        return pool_path(job.pool_id, job.pool_name, job.pool_index)

    if mode == "pool":
        if not post.pools:
            return base / "pools" / "_no_pool" / fname
        pid = post.pools[0]
        pool = pool_lookup(pid)
        if pool is None:
            return pool_path(pid, None, None)
        idx = pool.post_ids.index(post.id) + 1 if post.id in pool.post_ids else None
        return pool_path(pid, pool.display_name, idx)

    if mode == "search":
        return base / "searches" / sanitize(job.source or "_no_tags") / fname

    if mode == "artist":
        names = post.artists
        return base / "artists" / sanitize(names[0] if names else "_unknown_artist") / fname

    if mode in ("copyright", "character", "species"):
        vals = post.tags.get(mode, [])
        sub = {"copyright": "copyrights", "character": "characters", "species": "species"}[mode]
        return base / sub / sanitize(vals[0] if vals else f"_unknown_{mode}") / fname

    if mode == "rating":
        return base / "ratings" / RATING_NAMES.get(post.rating, "unknown") / fname

    return base / fname  # flat


class History:
    """Remembers which posts were downloaded so they can be skipped/marked."""

    def __init__(self, path: Path):
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            if not path.exists():
                old_path = path.parent.parent / "e621-grabber" / path.name
                if old_path.exists():
                    try:
                        import shutil
                        shutil.copy2(old_path, path)
                    except OSError:
                        pass
            db_target = str(path)
        except OSError:
            db_target = ":memory:"
        self._lock = threading.Lock()
        self._conn = sqlite3.connect(db_target, check_same_thread=False)
        self._conn.execute(
            "CREATE TABLE IF NOT EXISTS downloads ("
            "post_id INTEGER PRIMARY KEY, md5 TEXT, path TEXT, downloaded_at REAL)"
        )
        self._conn.commit()
        self._ids = {r[0] for r in self._conn.execute("SELECT post_id FROM downloads")}

    def has(self, post_id: int) -> bool:
        return post_id in self._ids

    def add(self, post: Post, path: Path) -> None:
        with self._lock:
            self._conn.execute(
                "INSERT OR REPLACE INTO downloads VALUES (?, ?, ?, ?)",
                (post.id, post.md5, str(path), time.time()),
            )
            self._conn.commit()
            self._ids.add(post.id)

    def path_for(self, post_id: int) -> str | None:
        with self._lock:
            row = self._conn.execute(
                "SELECT path FROM downloads WHERE post_id = ?", (post_id,)).fetchone()
        return row[0] if row else None

    def clear(self) -> None:
        with self._lock:
            self._conn.execute("DELETE FROM downloads")
            self._conn.commit()
            self._ids.clear()

    def __len__(self) -> int:
        return len(self._ids)


class SpankBank:
    """Persistent bookmark store for online-viewable posts (no full local files)."""

    def __init__(self, path: Path):
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            db_target = str(path)
        except OSError:
            db_target = ":memory:"
        self._lock = threading.Lock()
        self._conn = sqlite3.connect(db_target, check_same_thread=False)
        self._conn.execute(
            "CREATE TABLE IF NOT EXISTS spank_bank ("
            "post_id INTEGER PRIMARY KEY, "
            "data_json TEXT, "
            "added_at REAL)"
        )
        self._conn.commit()
        self._ids = {r[0] for r in self._conn.execute("SELECT post_id FROM spank_bank")}

    def has(self, post_id: int) -> bool:
        return post_id in self._ids

    def add(self, post: Post) -> None:
        with self._lock:
            data = json.dumps(asdict(post))
            self._conn.execute(
                "INSERT OR REPLACE INTO spank_bank VALUES (?, ?, ?)",
                (post.id, data, time.time()),
            )
            self._conn.commit()
            self._ids.add(post.id)

    def remove(self, post_id: int) -> None:
        with self._lock:
            self._conn.execute("DELETE FROM spank_bank WHERE post_id = ?", (post_id,))
            self._conn.commit()
            self._ids.discard(post_id)

    def toggle(self, post: Post) -> bool:
        if self.has(post.id):
            self.remove(post.id)
            return False
        else:
            self.add(post)
            return True

    def get_all(self) -> list[Post]:
        with self._lock:
            cursor = self._conn.execute("SELECT data_json FROM spank_bank ORDER BY added_at DESC")
            posts = []
            for (raw,) in cursor:
                try:
                    d = json.loads(raw)
                    posts.append(Post(**d))
                except Exception:
                    pass
            return posts

    def clear(self) -> None:
        with self._lock:
            self._conn.execute("DELETE FROM spank_bank")
            self._conn.commit()
            self._ids.clear()

    def __len__(self) -> int:
        return len(self._ids)

