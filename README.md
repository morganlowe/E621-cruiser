<p align="center">
  <img src="assets/e621-cruiser.png" alt="E621 Cruiser Logo" width="128" height="128">
</p>

<h1 align="center">E621 Cruiser</h1>

<p align="center">
  <b>A fast, native desktop browser, gallery, and downloader for e621 built with Python &amp; PySide6 (Qt 6).</b><br>
  Tabbed tag &amp; pool browsing, instant autocomplete, floating hover previews, true full-screen viewer, automated slideshows, local caching, and one-click batch downloads.
</p>

<p align="center">
  <a href="https://github.com/morganlowe/E621-cruiser/releases/latest">
    <img src="https://img.shields.io/github/v/release/morganlowe/E621-cruiser?style=for-the-badge&color=007ec6" alt="Latest Release">
  </a>
  <a href="https://github.com/morganlowe/E621-cruiser/actions/workflows/release.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/morganlowe/E621-cruiser/release.yml?style=for-the-badge&label=Build" alt="Release Build">
  </a>
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20Linux-orange?style=for-the-badge" alt="Windows | Linux">
  <img src="https://img.shields.io/badge/License-MIT-purple?style=for-the-badge" alt="MIT License">
</p>

<p align="center">
  <a href="https://github.com/morganlowe/E621-cruiser/releases/latest"><b>Download Latest Release</b></a>
  &nbsp;·&nbsp;
  <a href="#download--install">Installation</a>
  &nbsp;·&nbsp;
  <a href="#features">Features</a>
  &nbsp;·&nbsp;
  <a href="#first-time-setup">First-Time Setup</a>
  &nbsp;·&nbsp;
  <a href="#keyboard-shortcuts">Shortcuts</a>
</p>

---

## Download & Install

Grab the prebuilt package for your OS from the [**Latest Release**](https://github.com/morganlowe/E621-cruiser/releases/latest) page:

| Platform | Package | Install / Run |
| :--- | :--- | :--- |
| **Windows (x64)** | `E621Cruiser-*-windows-x64.exe` | Standalone app — just double-click to run (no Python needed) |
| **Debian / Ubuntu** | `e621-cruiser_*_all.deb` | `sudo apt install ./e621-cruiser_*_all.deb` |
| **Arch / CachyOS / Manjaro** | `e621-cruiser-*-any.pkg.tar.zst` | `sudo pacman -U e621-cruiser-*-any.pkg.tar.zst` |
| **Fedora** | `e621-cruiser-*.noarch.rpm` | `sudo dnf install ./e621-cruiser-*.noarch.rpm` |

### Platform Notes

- **Windows**: The `.exe` bundles Python and Qt so nothing else needs to be installed. Because it is not code-signed, Windows SmartScreen may show a prompt on first launch — click **More info → Run anyway**.
- **Debian / Ubuntu**: Installs `e621-cruiser` to your application menu and `PATH`, automatically pulling in `python3-pyside6` and `python3-requests`. Requires Ubuntu 24.04+ / Debian 12+.
- **Arch / CachyOS**: Install the `.pkg.tar.zst` directly with `pacman -U`, or download the attached `PKGBUILD` from the release and run `makepkg -si`.
- **Fedora**: `sudo dnf install ./e621-cruiser-*.noarch.rpm` installs the launcher, desktop entry, icons, and Python/PySide6 dependencies automatically.

<details>
<summary><b>Install from Source (Linux &amp; Windows)</b></summary>

#### Linux
```bash
git clone https://github.com/morganlowe/E621-cruiser.git
cd E621-cruiser
./install.sh
```
This creates `.venv`, installs dependencies, and adds **E621 Cruiser** to your application menu, Desktop, and `~/.local/bin/e621-cruiser`. Run `./uninstall.sh` to remove shortcuts.

#### Windows
1. Install [Python 3.10+](https://www.python.org/downloads/) with **"Add python.exe to PATH"** checked.
2. Clone and run the installer:
   ```cmd
   git clone https://github.com/morganlowe/E621-cruiser.git
   cd E621-cruiser
   install.bat
   ```
   Launch anytime from the Desktop / Start Menu shortcut or `run.bat`.

</details>

---

## Features

### Browsing & Search
- **Multi-Tab Browsing & History**: Open tag searches, artists, and pools in separate tabs (`Ctrl+T` / `Ctrl+W`) with per-tab Back/Forward history (`Alt+Left` / `Alt+Right`).
- **Instant Tag Autocomplete**: Pre-seeded with popular e621 tags and learns new tags as you browse without overwriting existing query tokens.
- **Rich Search Filters**: Full support for `-tag` exclusions, `~tag` OR queries, metatags (`score:>100`, `artist:name`), ratings (`Safe` / `Questionable` / `Explicit`), media types, `Pools only`, and sort orders.
- **Color-Coded Tag Inspector**: Click any tag to add/toggle it in your search; right-click to exclude or isolate. Click any artist name to browse their full gallery.
- **Quick Blacklist & Client-Side Filtering**: Collapsible one-click exclusions (Gore, Scat, Watersports, Young/Cub, Flash) plus full e621-syntax client-side blacklist rules persisted across restarts.

### Viewer, Caching & Slideshows
- **Hover & True Full-Screen Viewer**: Floating preview card on hover; click to open the full-screen viewer with edge-click and keyboard navigation (`Left`/`Right`/`A`/`D`), `F11` true fullscreen, and toggleable tag sidebar (`T`).
- **Local Disk & Memory Image Cache**: Checks downloaded library files and local cache before re-fetching thumbnails or previews, and reuses cached full-size images during downloads.
- **Automated Slideshows**: Play any search, pool, artist, or Spank Bank collection with configurable intervals (`1–300s`), reading order or shuffle mode, and `Space` to pause/resume.

### Spank Bank & Direct e621 Sync
- **Spank Bank (Online Bookmarks)**: Bookmark **Posts**, **Pools**, and **Artists** in a persistent local database without downloading files. Includes instant filtering, a combined **Latest From All Saved Artists** feed, and batch downloading organized by pool or artist.
- **Credit Artists on e621**: Favorite (`L`) and upvote (`U`) posts directly from the inspector, viewer, or right-click menu — synced straight to your e621 account so artists receive credit and favorites appear in `fav:yourusername`.

### Smart Batch Downloads
- **Pools in Reading Order**: Downloads multi-page comics sequentially as `0001_<id>.ext`, `0002_<id>.ext`, etc.
- **Flexible Folder Organization**: Organize downloads by `pool`, `artist`, `search`, `species`, `character`, `copyright`, `rating`, or `flat`, with optional `.txt` tag sidecars.
- **SQLite History & Duplicate Checking**: Tracks downloaded post IDs, verifies MD5 checksums, checks disk before skipping, and prunes deleted files from history.
- **API Compliant**: Supports e621's v2 posts API (`mode=extended`) with built-in 1 req/sec rate limiting and a connection diagnostic tester.

---

## First-Time Setup

E621 Cruiser works out of the box in **Anonymous Mode**, or you can connect your e621 account to unlock restricted posts, private favorites, and live favoriting/upvoting:

1. Open **File → Settings** (`Ctrl+,`).
2. Create an API key at [e621.net → Account → Manage API Access](https://e621.net/users/home).
3. Enter your **Username** and **API Key**, then click **Test Connection**.
4. *(Optional)* Add client-side blacklist rules, choose your default download folder, and set parallel download threads (`1–8`).

| File | Linux | Windows |
| :--- | :--- | :--- |
| Settings | `~/.config/e621-cruiser/settings.json` | `%APPDATA%\e621-cruiser\settings.json` |
| History, Spank Bank & Cache | `~/.local/share/e621-cruiser/` | `%LOCALAPPDATA%\e621-cruiser\` |

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl+F` | Focus tag search bar |
| `Enter` | Submit tag search / pool query |
| `Tab` | Autocomplete active tag and add space |
| `Up / Down` | Navigate autocomplete suggestions |
| `Escape` | Close autocomplete popup or viewer |
| `Left / Right` or `A / D` | Previous / Next image in viewer |
| `Space` | Play / Pause slideshow |
| `F11` | Toggle true fullscreen |
| `T` | Toggle tag sidebar in viewer |
| `L` | Favorite / unfavorite post on e621 |
| `U` | Upvote / remove upvote on e621 (`+1`) |
| `B` | Add / remove post in Spank Bank |
| `Ctrl+D` | Download selected posts |
| `Ctrl+T` / `Ctrl+W` | New tab / Close tab |
| `Alt+Left` / `Alt+Right` | Tab history Back / Forward |
| `Ctrl+,` | Open Settings |
| `Ctrl+Q` | Quit |

---

## Download Directory Structure

```text
<download_dir>/
├── pools/
│   └── <pool_name> (<id>)/
│       ├── 0001_<id>.<ext>
│       └── 0002_<id>.<ext>
├── artists/
│   └── <artist>/
│       └── <id>.<ext>
├── searches/
│   └── <tags>/
│       └── <id>.<ext>
├── species/ / characters/ / copyrights/ / ratings/ ...
└── flat/
```

---

## Development & Packaging

Run the test suite:
```bash
.venv/bin/python -m unittest discover -s tests -v
```

Packaging definitions live in [`packaging/`](packaging/) (`.deb`, Arch `PKGBUILD`, Fedora `.spec`, and Windows PyInstaller launcher) and are built automatically on every `v*` tag push via [`.github/workflows/release.yml`](.github/workflows/release.yml).

---

## License

Distributed under the [MIT License](LICENSE).
