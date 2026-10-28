"""Persistent settings stored as JSON in the XDG config directory."""
from __future__ import annotations

import json
import os
import sys
from dataclasses import asdict, dataclass, field, fields
from pathlib import Path

if sys.platform == "win32":
    _CONFIG_BASE = Path(os.environ.get("APPDATA", Path.home() / "AppData" / "Roaming"))
    _DATA_BASE = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData" / "Local"))
else:
    _CONFIG_BASE = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    _DATA_BASE = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share"))

CONFIG_DIR = _CONFIG_BASE / "e621-cruiser"
DATA_DIR = _DATA_BASE / "e621-cruiser"
SETTINGS_FILE = CONFIG_DIR / "settings.json"

OLD_CONFIG_DIR = _CONFIG_BASE / "e621-grabber"
OLD_DATA_DIR = _DATA_BASE / "e621-grabber"

MIN_SCORE_ANY = -1000  # spin-box sentinel meaning "no score filter"

MEDIA_FILTERS: list[tuple[str, str]] = [
    ("All media", ""),
    ("Drawings / Static only", "-animated"),
    ("Animations only", "animated"),
    ("Videos only (WebM/MP4)", "type:webm"),
    ("GIFs only", "type:gif"),
]

QUICK_BLACKLIST_ITEMS: list[tuple[str, str]] = [
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
]

DEFAULT_BLACKLIST = """\
# One rule per line, e621 syntax. Examples:
#   gore
#   scat
#   young -rating:s
#   type:swf
"""


def full_blacklist(settings: Settings) -> str:
    """Combine custom blacklist text and active quick blacklist items."""
    parts = [settings.blacklist.strip()] if settings.blacklist.strip() else []
    if settings.quick_blacklist:
        parts.append("# Quick blacklist selections:")
        parts.extend(settings.quick_blacklist)
    return "\n".join(parts)


@dataclass
class Settings:
    username: str = ""
    api_key: str = ""
    blacklist: str = DEFAULT_BLACKLIST
    download_dir: str = str(Path.home() / "Pictures" / "e621")
    organize_mode: str = "search"
    write_tags_file: bool = False
    skip_downloaded: bool = True
    concurrent_downloads: int = 3
    rating_s: bool = True
    rating_q: bool = True
    rating_e: bool = True
    min_score: int = MIN_SCORE_ANY
    order: str = ""
    media_filter: str = ""
    pool_only: bool = False
    quick_blacklist: list[str] = field(default_factory=list)
    expand_blacklist: bool = False
    expand_saved: bool = False
    saved_searches: list[str] = field(default_factory=list)
    favorites_user: str = ""
    window_geometry: str = ""
    splitter_state: str = ""

    @classmethod
    def from_dict(cls, data: dict) -> "Settings":
        known = {f.name for f in fields(cls)}
        return cls(**{k: v for k, v in data.items() if k in known})

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def load(cls, path: Path | None = None) -> "Settings":
        file_to_read = path or SETTINGS_FILE
        if path is None and not file_to_read.exists() and (OLD_CONFIG_DIR / "settings.json").exists():
            file_to_read = OLD_CONFIG_DIR / "settings.json"
        try:
            data = json.loads(file_to_read.read_text())
        except (OSError, ValueError):
            inst = cls()
            object.__setattr__(inst, "_file", path)
            return inst
        inst = cls.from_dict(data)
        object.__setattr__(inst, "_file", path)
        return inst

    def save(self, path: Path | None = None) -> None:
        target = path or getattr(self, "_file", None) or SETTINGS_FILE
        try:
            target.parent.mkdir(parents=True, exist_ok=True)
            tmp = target.with_suffix(".tmp")
            data = {f.name: getattr(self, f.name) for f in fields(self)}
            tmp.write_text(json.dumps(data, indent=2))
            os.chmod(tmp, 0o600)  # file contains the API key
            os.replace(tmp, target)
        except OSError:
            pass
