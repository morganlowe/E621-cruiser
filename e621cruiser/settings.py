"""Persistent settings stored as JSON in the XDG config directory."""
from __future__ import annotations

import json
import os
from dataclasses import asdict, dataclass, field, fields
from pathlib import Path

_CONFIG_BASE = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
_DATA_BASE = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share"))

CONFIG_DIR = _CONFIG_BASE / "e621-cruiser"
DATA_DIR = _DATA_BASE / "e621-cruiser"
SETTINGS_FILE = CONFIG_DIR / "settings.json"

OLD_CONFIG_DIR = _CONFIG_BASE / "e621-grabber"
OLD_DATA_DIR = _DATA_BASE / "e621-grabber"

MIN_SCORE_ANY = -1000  # spin-box sentinel meaning "no score filter"

DEFAULT_BLACKLIST = """\
# One rule per line, e621 syntax. Examples:
#   gore
#   scat
#   young -rating:s
#   type:swf
"""


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
    saved_searches: list[str] = field(default_factory=list)
    favorites_user: str = ""
    window_geometry: str = ""

    @classmethod
    def load(cls) -> "Settings":
        file_to_read = SETTINGS_FILE
        if not file_to_read.exists() and (OLD_CONFIG_DIR / "settings.json").exists():
            file_to_read = OLD_CONFIG_DIR / "settings.json"
        try:
            data = json.loads(file_to_read.read_text())
        except (OSError, ValueError):
            return cls()
        known = {f.name for f in fields(cls)}
        return cls(**{k: v for k, v in data.items() if k in known})

    def save(self) -> None:
        try:
            CONFIG_DIR.mkdir(parents=True, exist_ok=True)
            tmp = SETTINGS_FILE.with_suffix(".tmp")
            tmp.write_text(json.dumps(asdict(self), indent=2))
            os.chmod(tmp, 0o600)  # file contains the API key
            os.replace(tmp, SETTINGS_FILE)
        except OSError:
            pass
