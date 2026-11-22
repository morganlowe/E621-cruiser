<p align="center">
  <img src="assets/e621-cruiser.png" alt="E621 Cruiser Logo" width="128" height="128">
</p>

<h1 align="center">E621 Cruiser</h1>

<p align="center">
  <b>A fast, self-contained native desktop browser, gallery, and downloader for e621 built in Rust with egui &amp; eframe.</b><br>
  Single native binary with zero Python, Qt, GTK, OpenSSL, or system SQLite runtime dependencies — plus tabbed tag &amp; pool browsing, instant autocomplete, floating hover previews, true full-screen viewer, automated slideshows, local caching, and batch downloads.
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
  <a href="#whats-new-in-v20">What's New</a>
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

## What's New in v2.0

E621 Cruiser 2.0 is a **complete ground-up rewrite in Rust** (`egui` + `eframe`), replacing the v1.x Python + PySide6 stack with a single self-contained native executable:

- **Zero Runtime Dependencies**: Compiles into one native binary with bundled TLS (`rustls`), SQLite (`rusqlite`), native file dialogs (`rfd`), and embedded assets — no Python, Qt, GTK, OpenSSL, or system SQLite setup required across Linux distros or Windows.
- **First-Run API Key Onboarding Splash**: Presents e621 username and API key fields with step-by-step setup instructions, a direct button to `e621.net → Account → Manage API Access`, a Show/Hide API key toggle, inline **Test Connection**, and **Skip for now (Browse Anonymously)** (re-openable anytime from **File → API Key Setup…**).
- **Drop-In v1.x Data Migration**: Automatically reuses your existing `settings.json`, `history.db`, `spank_bank.db`, and image cache (including legacy `e621-grabber` folders), and skips the first-run splash automatically if credentials are already configured.
- **UI & Ergonomic Polish**: Non-blocking async connection testing in Settings & Setup, persisted window dimensions and panel splitter widths, responsive layout across window sizes, dynamic viewer window titles, and a new **portable Linux `.tar.gz`** alongside `.deb`, `.pkg.tar.zst`, `.rpm`, and standalone Windows `.exe`.

---

## Download & Install

Grab the prebuilt native package for your OS from the [**Latest Release**](https://github.com/morganlowe/E621-cruiser/releases/latest) page:

| Platform | Package | Install / Run |
| :--- | :--- | :--- |
| **Windows (x64)** | `E621Cruiser-*-windows-x64.exe` | Standalone native executable — just double-click to run |
| **Debian / Ubuntu** | `e621-cruiser_*_amd64.deb` | `sudo apt install ./e621-cruiser_*_amd64.deb` |
| **Arch / CachyOS / Manjaro** | `e621-cruiser-*-x86_64.pkg.tar.zst` | `sudo pacman -U e621-cruiser-*-x86_64.pkg.tar.zst` |
| **Fedora** | `e621-cruiser-*.x86_64.rpm` | `sudo dnf install ./e621-cruiser-*.x86_64.rpm` |
| **Portable Linux (x86_64)** | `e621-cruiser-*-linux-x86_64.tar.gz` | Extract and run `./e621-cruiser` on any modern Linux distro |

### Platform Notes

- **Windows**: Single standalone `.exe` compiled from Rust with bundled TLS (`rustls`), SQLite, and UI — nothing else needs to be installed. Because it is not code-signed, Windows SmartScreen may show a prompt on first launch — click **More info → Run anyway**.
- **Debian / Ubuntu**: Installs `e621-cruiser` to your application menu and `/usr/bin/e621-cruiser` with minimal system C runtime dependencies (`libc6`, `libgcc-s1`). Requires Ubuntu 22.04+ / Debian 12+.
- **Arch / CachyOS**: Install the `.pkg.tar.zst` directly with `pacman -U`, or download the attached `PKGBUILD` from the release and run `makepkg -si`.
- **Fedora**: `sudo dnf install ./e621-cruiser-*.x86_64.rpm` installs the native binary, desktop entry, and icons.
- **Portable Linux**: Built on Ubuntu 22.04 (`glibc 2.35+`) so it runs directly across distributions without dependency conflicts.

<details>
<summary><b>Build &amp; Install from Source (Linux &amp; Windows)</b></summary>

Requires the [Rust toolchain (`cargo` / `rustc`)](https://rustup.rs/).

#### Linux
```bash
git clone https://github.com/morganlowe/E621-cruiser.git
cd E621-cruiser
./install.sh
```
This compiles `target/release/e621-cruiser`, installs it to `~/.local/bin/e621-cruiser`, and adds **E621 Cruiser** to your application menu and Desktop. Run `./uninstall.sh` to remove binary and shortcuts.

#### Windows
1. Install [Rust (`rustup`)](https://rustup.rs/).
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
- **Hover & True Full-Screen Viewer**: Floating preview card on hover; click or double-click to open the full-screen viewer with edge-click and keyboard navigation (`Left`/`Right`/`A`/`D`), `F11` / `F` true fullscreen, and toggleable tag sidebar (`T`).
- **Local Disk & Memory Image Cache**: Checks downloaded library files and local cache before re-fetching thumbnails or previews, and reuses cached full-size images during downloads.
- **Automated Slideshows**: Play any search, pool, artist, or Spank Bank collection with configurable intervals (`0.5–300s`), live `+` / `-` speed controls, reading order or shuffle mode, and `Space` to pause/resume.

### Spank Bank & Direct e621 Sync
- **Spank Bank (Online Bookmarks)**: Bookmark **Posts**, **Pools**, and **Artists** in a persistent local SQLite database without downloading files. Includes instant filtering, a combined **Latest From All Saved Artists** feed, and batch downloading organized by pool or artist.
- **Credit Artists on e621**: Favorite (`L`) and upvote (`U`) posts directly from the inspector, viewer, or right-click menu — synced straight to your e621 account so artists receive credit and favorites appear in `fav:yourusername`.

### Smart Batch Downloads
- **Pools in Reading Order**: Downloads multi-page comics sequentially as `0001_<id>.ext`, `0002_<id>.ext`, etc.
- **Flexible Folder Organization**: Organize downloads by `pool`, `artist`, `search`, `species`, `character`, `copyright`, `rating`, or `flat`, with optional `.txt` tag sidecars.
- **SQLite History & Duplicate Checking**: Tracks downloaded post IDs, verifies MD5 checksums, checks disk before skipping, and prunes deleted files from history.
- **API Compliant**: Supports e621's v2 posts API (`mode=extended`) with built-in 1 req/sec rate limiting and a connection diagnostic tester.

---

## First-Time Setup

On your first launch without saved credentials, E621 Cruiser greets you with an **API Key Setup** splash screen explaining how to connect your account:

1. Sign in to [e621.net](https://e621.net) in your browser and open **Account → Manage API Access** (or click **Open e621 Account Page** on the splash screen).
2. Re-enter your password when prompted, then view or generate your API key.
3. Enter your **e621 Username** (not email) and **API Key**, click **Test Connection** to verify, and press **Save & Continue**.
4. Prefer not to log in right away? Click **Skip for now (Browse Anonymously)** — you can return anytime via **File → API Key Setup…** or **File → Settings** (`Ctrl+,`) to set up credentials, blacklist rules, download folders, and parallel download threads (`1–8`).

Existing `settings.json`, `history.db`, `spank_bank.db`, and cached images from v1.x are automatically preserved (and the first-run splash is skipped automatically when credentials are already present):

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
| `+ / -` | Increase / decrease slideshow speed in viewer |
| `F11` or `F` | Toggle true fullscreen |
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

Run the test suite and linter:
```bash
cargo test
cargo clippy --all-targets
```

Packaging definitions live in [`packaging/`](packaging/) (`.deb`, Arch `PKGBUILD`, Fedora `.spec`, and Windows icon resource via `build.rs`) and are built automatically on every `v*` tag push via [`.github/workflows/release.yml`](.github/workflows/release.yml).

---

## License

Distributed under the [MIT License](LICENSE).
