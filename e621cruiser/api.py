"""Minimal, rate-limited client for the e621 JSON API.

API docs: https://e621.net/help/api
e621 requires a descriptive User-Agent and asks clients to stay under
~2 requests/second. We default to 1 request/second for API calls.
"""
from __future__ import annotations

import threading
import time
from dataclasses import dataclass, field
from typing import Callable

import requests

BASE_URL = "https://e621.net"
APP_NAME = "E621Cruiser"
APP_VERSION = "1.0"
POSTS_PER_PAGE = 320  # API maximum
MAX_PAGE = 750        # numeric page limit enforced by e621

TAG_CATEGORIES = {
    0: "general", 1: "artist", 3: "copyright", 4: "character",
    5: "species", 6: "invalid", 7: "meta", 8: "lore",
}

# Tags that live in the "artist" category but aren't actual artists.
NON_ARTIST_TAGS = {
    "conditional_dnp", "sound_warning", "epilepsy_warning", "unknown_artist",
    "anonymous_artist", "avoid_posting", "third-party_edit", "jumpscare_warning",
    "unknown_artist_signature", "artist_request", "censored_artist",
}


def user_agent(username: str = "") -> str:
    who = username.strip() or "anonymous"
    return f"{APP_NAME}/{APP_VERSION} (by {who} on e621)"


class ApiError(Exception):
    pass


class RateLimiter:
    """Thread-safe minimum interval between calls."""

    def __init__(self, interval: float):
        self.interval = interval
        self._lock = threading.Lock()
        self._last = 0.0

    def wait(self) -> None:
        with self._lock:
            delta = time.monotonic() - self._last
            if delta < self.interval:
                time.sleep(self.interval - delta)
            self._last = time.monotonic()


@dataclass
class Tag:
    name: str
    category: int
    post_count: int

    @property
    def category_name(self) -> str:
        return TAG_CATEGORIES.get(self.category, "general")


@dataclass
class Post:
    id: int
    md5: str
    ext: str
    file_url: str | None
    preview_url: str | None
    sample_url: str | None
    width: int
    height: int
    size: int
    rating: str
    score: int
    fav_count: int
    tags: dict[str, list[str]] = field(default_factory=dict)
    pools: list[int] = field(default_factory=list)
    description: str = ""

    @classmethod
    def from_json(cls, d: dict) -> "Post":
        f = d.get("file") or {}
        preview = d.get("preview") or {}
        sample = d.get("sample") or {}
        score = d.get("score") or {}
        return cls(
            id=int(d["id"]),
            md5=f.get("md5") or "",
            ext=f.get("ext") or "",
            # file.url is null for deleted posts and, for anonymous users,
            # for posts hidden by e621's global blacklist (log in to fix).
            file_url=f.get("url"),
            preview_url=preview.get("url"),
            sample_url=sample.get("url") if sample.get("has") else f.get("url"),
            width=int(f.get("width") or 0),
            height=int(f.get("height") or 0),
            size=int(f.get("size") or 0),
            rating=d.get("rating") or "",
            score=int(score.get("total") or 0),
            fav_count=int(d.get("fav_count") or 0),
            tags={k: list(v) for k, v in (d.get("tags") or {}).items()},
            pools=list(d.get("pools") or []),
            description=d.get("description") or "",
        )

    @property
    def all_tags(self) -> set[str]:
        out: set[str] = set()
        for v in self.tags.values():
            out.update(v)
        return out

    @property
    def artists(self) -> list[str]:
        return [a for a in self.tags.get("artist", []) if a not in NON_ARTIST_TAGS]

    @property
    def is_video(self) -> bool:
        return self.ext in ("webm", "mp4")

    @property
    def page_url(self) -> str:
        return f"{BASE_URL}/posts/{self.id}"


@dataclass
class Pool:
    id: int
    name: str
    post_ids: list[int]
    category: str = ""
    description: str = ""
    is_active: bool = True

    @classmethod
    def from_json(cls, d: dict) -> "Pool":
        return cls(
            id=int(d["id"]),
            name=d.get("name") or f"pool_{d['id']}",
            post_ids=[int(x) for x in d.get("post_ids") or []],
            category=d.get("category") or "",
            description=d.get("description") or "",
            is_active=bool(d.get("is_active", True)),
        )

    @property
    def display_name(self) -> str:
        return self.name.replace("_", " ")

    @property
    def post_count(self) -> int:
        return len(self.post_ids)

    @property
    def page_url(self) -> str:
        return f"{BASE_URL}/pools/{self.id}"


class E621Client:
    def __init__(self, username: str = "", api_key: str = "", interval: float = 1.0):
        self.username = username.strip()
        self.session = requests.Session()
        self.session.headers["User-Agent"] = user_agent(self.username)
        self.session.headers["Accept"] = "application/json"
        if self.username and api_key.strip():
            self.session.auth = (self.username, api_key.strip())
        self._limiter = RateLimiter(interval)
        self._autocomplete_cache: dict[str, list[Tag]] = {}

    @property
    def authenticated(self) -> bool:
        return self.session.auth is not None

    def check_connection(self) -> tuple[bool, str]:
        """Verify API connectivity and authentication status."""
        try:
            if self.authenticated:
                data = self._get("/users.json", {"search[name_matches]": self.username}, retries=1)
                if isinstance(data, list) and data:
                    user_info = data[0]
                    level = user_info.get("level_string", "Member")
                    fav_count = user_info.get("favorite_count", 0)
                    return True, f"Logged in as {self.username} ({level}) · {fav_count} favorites"
                return True, f"Connected to e621 as {self.username}"
            else:
                self._get("/posts.json", {"limit": 1}, retries=1)
                return True, "Connected to e621 (Anonymous mode)"
        except Exception as e:
            return False, f"Connection failed: {e}"

    # ------------------------------------------------------------------ core
    def _get(self, path: str, params: dict | None = None, retries: int = 3):
        for attempt in range(retries + 1):
            self._limiter.wait()
            try:
                r = self.session.get(BASE_URL + path, params=params, timeout=30)
            except requests.RequestException as e:
                if attempt == retries:
                    raise ApiError(f"Network error: {e}") from e
                time.sleep(2 ** attempt)
                continue
            if r.status_code in (429, 500, 502, 503, 504, 520, 521, 522) and attempt < retries:
                time.sleep(2 ** (attempt + 1))
                continue
            if r.status_code == 401:
                raise ApiError("Authentication failed. Check your username and API key in Settings.")
            if r.status_code == 403:
                raise ApiError("Access denied (403). e621 may be blocking this request.")
            if r.status_code == 404:
                raise ApiError("Not found (404).")
            if not r.ok:
                raise ApiError(f"HTTP {r.status_code}: {r.text[:200]}")
            try:
                return r.json()
            except ValueError as e:
                raise ApiError("Invalid JSON response from e621.") from e
        raise ApiError("Request failed after retries.")

    # ----------------------------------------------------------------- posts
    def search_posts(self, tags: str, page: int | str = 1, limit: int = POSTS_PER_PAGE) -> list[Post]:
        data = self._get("/posts.json", {"tags": tags, "page": page, "limit": limit})
        return [Post.from_json(p) for p in data.get("posts", [])]

    def fetch_all_posts(
        self,
        tags: str,
        start_page: int = 1,
        progress: Callable[[int, int], None] | None = None,
        should_stop: Callable[[], bool] | None = None,
    ) -> list[Post]:
        """Fetch every page from start_page until results run out."""
        out: list[Post] = []
        page = start_page
        while page <= MAX_PAGE:
            if should_stop and should_stop():
                break
            batch = self.search_posts(tags, page)
            out.extend(batch)
            if progress:
                progress(page, len(out))
            if len(batch) < POSTS_PER_PAGE:
                break
            page += 1
        return out

    # ------------------------------------------------------------------ tags
    def autocomplete(self, prefix: str) -> list[Tag]:
        prefix = prefix.strip().lower()
        if not prefix:
            return []
        if prefix in self._autocomplete_cache:
            return self._autocomplete_cache[prefix]
        res: list[Tag] = []
        if len(prefix) >= 3:
            try:
                data = self._get("/tags/autocomplete.json",
                                 {"search[name_matches]": prefix, "expiry": 7}, retries=1)
                if isinstance(data, list):
                    res = [Tag(t["name"], int(t.get("category", 0)), int(t.get("post_count", 0)))
                           for t in data]
            except ApiError:
                pass  # fall back to the plain tag search below
        if not res:
            try:
                data = self._get("/tags.json", {
                    "search[name_matches]": prefix + "*",
                    "search[order]": "count",
                    "limit": 15,
                }, retries=1)
                if isinstance(data, list):
                    res = [Tag(t["name"], int(t.get("category", 0)), int(t.get("post_count", 0))) for t in data]
            except ApiError:
                pass
        if res:
            self._autocomplete_cache[prefix] = res
        return res

    # ----------------------------------------------------------------- pools
    def search_pools(self, query: str, page: int = 1) -> list[Pool]:
        query = query.strip()
        if query.isdigit():
            return [self.get_pool(int(query))]
        q = query.replace(" ", "_")
        if "*" not in q:
            q = f"*{q}*"
        data = self._get("/pools.json", {
            "search[name_matches]": q,
            "search[order]": "updated_at",
            "limit": 75,
            "page": page,
        })
        if not isinstance(data, list):
            return []
        return [Pool.from_json(p) for p in data]

    def get_pool(self, pool_id: int) -> Pool:
        return Pool.from_json(self._get(f"/pools/{pool_id}.json"))

    def get_pool_posts(self, pool: Pool) -> list[Post]:
        """Return the pool's posts in pool order."""
        found: dict[int, Post] = {}
        page = 1
        while page <= MAX_PAGE:
            batch = self.search_posts(f"pool:{pool.id}", page)
            for p in batch:
                found[p.id] = p
            if len(batch) < POSTS_PER_PAGE or len(found) >= pool.post_count:
                break
            page += 1
        return [found[i] for i in pool.post_ids if i in found]
