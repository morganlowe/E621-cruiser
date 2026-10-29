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
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20Linux-orange.svg" alt="Windows | Linux">
  <img src="https://img.shields.io/badge/License-MIT-purple.svg" alt="MIT License">
</p>

---

## Highlights

- **Direct e621 Site Interactions (Credit Artists & Like)**:
  - **Favorite (Like)**: One-click favorite directly synced to your e621 account via `POST /favorites.json`. Credits the artist, increments site-wide favorite tallies, and adds the post to your personal e621 favorites (`fav:username`).
  - **Upvote (+1)**: Direct score voting via `POST /posts/<id>/votes.json` to boost post visibility in site-wide top-rated searches.
  - **Live Indicators**: Shows current favorite counts and score badges in the Inspector and full-screen viewer.
- **Spank Bank (Online Bookmarks & Slideshows)**:
  - Save posts and entire multi-page pools to a persistent collection for online viewing without downloading files locally.
  - Dedicated **Pools** and **Posts** tabs with instant filtering by title, artist, tag, or ID.
  - Full-screen automated slideshow playback with custom timers (1–300s), reading order, and randomized shuffle modes.
- **Tabbed Browsing & Navigation History**:
  - Open searches and pools in separate tabs (`Ctrl+T`, `Ctrl+W`).
  - Backward and forward history navigation (`Alt+Left`, `Alt+Right`) per tab with scroll position and query state retention.
- **Instant Tag Autocomplete**: Pre-seeded with top e621 tags and learns dynamically as you browse. Never overwrites existing query tokens.
- **Tag Inspector & One-Click Appending**: Color-coded categorized tags (Artist, Species, Character, Copyright, General). Single-click to add/toggle tags in search; right-click to exclude (`-tag`) or isolate.
- **Search Filters & Media Filters**: Supports `-tag` (exclude), `~tag` (OR), metatags (`score:>100`, `artist:name`), ratings (Safe, Questionable, Explicit), media filters (drawings, animations, webm, gifs), pools only, and custom sort orders.
- **Collapsible Quick Blacklist**: One-click common exclusions (Gore, Scat, Watersports, Young/Cub, Flash) with instant disk persistence.
- **Hover & Full-Screen Image Viewer**:
  - **Hover**: Floating medium preview card directly over thumbnails.
  - **Click**: Full-screen modal viewer with keyboard navigation (`Left`/`Right`/`A`/`D`), fullscreen (`F11`), tag panel toggle (`T`), favorite (`L`), upvote (`U`), Spank Bank (`B`), slideshow (`Space`), and instant download (`D`).
- **Pools in Reading Order**: Browse pools by ID or name; one-click download preserves comic reading order (`0001_id.ext`, `0002_id.ext`).
- **User Favorites**: Load public or private favorites directly from your e621 account.
- **Client-Side Blacklist**: Filter out unwanted tags locally without leaking rules to the network.
- **SQLite Download History**: Tracks downloaded IDs to skip duplicates even across renames or directory changes; verifies MD5 checksums.
- **Flexible Organization Modes**: Organize files by search query, artist, pool, species, character, copyright, rating, or flat.
- **Cross-Platform (Windows & Linux)**: Native installers, desktop shortcuts, and dedicated launchers for both platforms.
- **API Rate Compliance**: Complies with e621's 1 req/sec rate limit policy and provides a built-in connection diagnostic tester.

---

## Installation & Setup

### Windows Setup

1. **Prerequisite**: Ensure [Python 3.10+](https://www.python.org/downloads/) is installed and **"Add python.exe to PATH"** was checked during installation.
2. **Download / Clone**:
   ```cmd
   git clone https://github.com/morganlowe/E621-cruiser.git
   cd E621-cruiser
   ```
3. **Install Shortcuts**: Double-click or run:
   ```cmd
   install.bat
   ```
   *This automatically creates the `.venv`, installs PySide6, and adds an **E621 Cruiser** shortcut on your Desktop and in your Start Menu.*
4. **Quick Launch**: Double-click `run.bat` or use the Desktop shortcut.

---

### Linux Setup

1. **Quick Install (Desktop & Menu Shortcut)**:
   ```bash
   git clone https://github.com/morganlowe/E621-cruiser.git
   cd E621-cruiser
   ./install.sh
   ```
   *Places an **E621 Cruiser** shortcut on your Desktop and in your application launcher (`~/.local/share/applications`).*
2. **Command Line / Direct Launch**:
   ```bash
   e621-cruiser     # from anywhere via ~/.local/bin
   ./run.sh         # or directly inside the repository
   ```
3. **Uninstall Shortcuts**:
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

## e621 Account Interactions & Crediting Artists

E621 Cruiser connects directly to e621's account interaction API, allowing you to support artists and manage your site-wide likes without opening a browser:

* **Favorite (Like)**:
  * Click **Favorite** in the sidebar inspector or viewer (or press `L`).
  * Sends an authenticated request to `POST /favorites.json` on e621.
  * Credits the artist on the site, increments the post's public `fav_count`, and stores the post in your personal e621 favorites (`fav:yourusername`).
  * Clicking again sends `DELETE /favorites/<id>.json` to remove the favorite.
* **Upvote (+1)**:
  * Click **▲ +1** in the sidebar inspector or viewer (or press `U`).
  * Sends an authenticated vote to `POST /posts/<id>/votes.json` on e621 to increase the post's total public score.
* **Grid Context Menu**: Right-click any thumbnail to favorite, upvote, download, or bookmark to the Spank Bank.

---

## Spank Bank & Slideshows

The **Spank Bank** provides a personal, persistent bookmarking system for online viewing without downloading files to disk:

* **Posts & Pools Separation**:
  * **Pools Tab**: Bookmark entire comics or themed series with post counts and reading order.
  * **Posts Tab**: Bookmark individual standalone illustrations and animations.
* **Full-Screen Automated Slideshows**:
  * Play whole pools or your entire saved post bank in full-screen mode.
  * Set timer intervals (1 to 300 seconds) with automated looping.
  * Choose between **Reading Order** (sequential) and **Shuffle** (randomized order).
  * Spacebar instantly pauses or resumes playback.
* **Instant Filtering**: Search through your bookmarked posts or pools by tag, title, artist, or post ID.

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl+F` | Focus tag search bar |
| `Enter` | Submit tag search / pool query |
| `Tab` | Autocomplete active tag and add space |
| `Down / Up` | Navigate autocomplete suggestions |
| `Escape` | Close autocomplete popup or image viewer |
| `Left / Right` or `A / D` | Navigate Previous / Next image in viewer |
| `Space` | Play / Pause slideshow in viewer |
| `L` | Toggle Favorite on e621 (credits artist) |
| `U` | Toggle Upvote on e621 (+1) |
| `B` | Toggle post in Spank Bank |
| `F11` | Toggle viewer fullscreen mode |
| `T` | Toggle tag sidebar in viewer |
| `Ctrl+T` | Open new search tab |
| `Ctrl+W` | Close current tab |
| `Alt+Left` | Navigate back in tab history |
| `Alt+Right` | Navigate forward in tab history |
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
