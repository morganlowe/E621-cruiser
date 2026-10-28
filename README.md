# E621 Cruiser

<p align="center">
  <img src="assets/e621-cruiser.png" alt="E621 Cruiser Logo" width="128" height="128">
</p>

<p align="center">
  <b>A desktop crawler and gallery app for e621 built with Python & PySide6 (Qt).</b><br>
  Browse posts, pools, and favorites with instant tag autocomplete, floating hover previews, full-screen viewer, and one-click batch saving.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Python-3.10+-blue.svg" alt="Python 3.10+">
  <img src="https://img.shields.io/badge/GUI-PySide6%20%28Qt6%29-green.svg" alt="PySide6">
  <img src="https://img.shields.io/badge/Platform-Linux-orange.svg" alt="Linux">
  <img src="https://img.shields.io/badge/License-MIT-purple.svg" alt="MIT License">
</p>

---

## Highlights

- ⚡ **Instant Tag Autocomplete**: Pre-seeded with top e621 tags and learns dynamically as you browse. Never overwrites existing query tokens.
- 🏷️ **Tag Inspector & One-Click Appending**: Color-coded categorized tags (Artist, Species, Character, Copyright, General). Single-click to add/toggle tags in search; right-click to exclude (`-tag`) or isolate.
- 🔍 **Rich Search Filters**: Supports `-tag` (exclude), `~tag` (OR), metatags (`score:>100`, `artist:name`), ratings (Safe, Questionable, Explicit), and custom sort orders.
- 🖼️ **Hover & Full-Screen Image Viewer**:
  - **Hover**: Floating medium preview card directly over thumbnails.
  - **Click**: Full-screen modal viewer with keyboard navigation (`Left`/`Right`/`A`/`D`), fullscreen (`F11`), tag panel toggle (`T`), and instant download (`D`).
- 📚 **Pools in Reading Order**: Browse pools by ID or name; one-click download preserves comic reading order (`0001_id.ext`, `0002_id.ext`).
- ⭐ **User Favorites**: Load public or private favorites directly from your e621 account.
- 🛡️ **Client-Side Blacklist**: Filter out unwanted tags locally without leaking rules to the network.
- 💾 **SQLite Download History**: Tracks downloaded IDs to skip duplicates even across renames or directory changes; verifies MD5 checksums.
- 📂 **Flexible Organization Modes**: Organize files by search query, artist, pool, species, character, copyright, rating, or flat.
- 🌐 **Polite API Client**: Complies strictly with e621's 1 req/sec rate limit policy and provides a built-in connection diagnostic tester.

---

## Installation & Setup

### Quick Install (Menu & Desktop Shortcut)

Run the included installer to automatically create your virtual environment, install icons, and add an application shortcut to your desktop and launcher menu:

```bash
git clone https://github.com/morganlowe/E621-cruiser.git
cd E621-cruiser
./install.sh
```

Once installed, you can launch E621 Cruiser from your desktop environment's application menu, from the desktop shortcut, or from the terminal:
```bash
e621-cruiser
```

### Manual Run

To run directly without installing system shortcuts:
```bash
./run.sh
```
*The launcher automatically initializes a `.venv` on the first run (reusing system PySide6 if available).*

### Uninstallation

To remove desktop shortcuts, icons, and command line links:
```bash
./uninstall.sh
```

---

## Configuration & Credentials

Open **File → Settings** (or press `Ctrl+,`):

1. **Username & API Key**:
   - Create an API key on [e621.net → Account → Manage API Access](https://e621.net/users/home).
   - Enter your credentials and click **Test Connection** to verify live connectivity.
   - *Note*: An API key unlocks posts that e621 restricts for anonymous guests and allows loading private favorites.
2. **Blacklist**: Enter e621-syntax blacklist rules (one rule per line).
3. **Storage & Concurrency**: Configure parallel download threads (1–8) and toggle saving `.txt` tag sidecars.

Settings are stored in `~/.config/e621-cruiser/settings.json` (mode `0600`).
The SQLite download history is kept in `~/.local/share/e621-cruiser/history.db`.

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl+F` | Focus tag search bar |
| `Enter` | Submit tag search / pool query |
| `Tab` | Autocomplete active tag and add space |
| `Down / Up` | Navigate autocomplete suggestions |
| `Escape` | Close autocomplete popup or image viewer |
| `Left / Right` or `A / D` | Navigate Previous / Next image in viewer |
| `F11` | Toggle viewer fullscreen mode |
| `T` | Toggle tag sidebar in viewer |
| `Ctrl+D` | Download selected images |
| `Ctrl+,` | Open Settings |
| `Ctrl+Q` | Quit application |

---

## Directory Organization Structure

Downloaded files are saved according to your selected organization mode:

```
<download_dir>/
├── searches/
│   └── <tags>/
│       └── <id>.<ext>
├── pools/
│   └── <pool_name> (<id>)/
│       ├── 0001_<id>.<ext>
│       └── 0002_<id>.<ext>
├── artists/
│   └── <artist>/
│       └── <id>.<ext>
├── species/ / characters/ / copyrights/ / ratings/ ...
└── flat/
```

---

## Development & Tests

Run the test suite:
```bash
.venv/bin/python -m unittest discover -s tests
```

---

## License

Distributed under the [MIT License](LICENSE).
