"""Main application window."""
from __future__ import annotations

import os
from dataclasses import dataclass
from functools import partial
from pathlib import Path

from PySide6.QtCore import QByteArray, Qt, QTimer, QUrl, Signal
from PySide6.QtGui import QAction, QDesktopServices, QIcon, QImage, QKeySequence
from PySide6.QtWidgets import (QApplication, QCheckBox, QComboBox, QFileDialog, QFormLayout,
                               QGroupBox, QHBoxLayout, QLabel, QLineEdit, QListWidget,
                               QListWidgetItem, QMainWindow, QMenu, QMessageBox,
                               QPlainTextEdit, QProgressBar, QPushButton, QSpinBox, QSplitter,
                               QTabWidget, QVBoxLayout, QWidget)

from .api import POSTS_PER_PAGE, E621Client, Pool, Post
from .blacklist import Blacklist
from .downloader import DownloadManager
from .settings import DATA_DIR, MIN_SCORE_ANY, Settings
from .storage import ORGANIZE_MODES, DownloadJob, History
from .widgets import PostGrid, PostInspector, PreviewDialog, SettingsDialog, TagLineEdit, TagListWidget
from .workers import Runner, fetch_image

ORDERS = [("Newest", ""), ("Highest score", "order:score"), ("Most favorited", "order:favcount"),
          ("Random", "order:random"), ("Oldest", "order:id")]


@dataclass
class Source:
    kind: str            # "search" | "pool" | "favorites"
    label: str           # used for folder names in "By search" mode
    query: str = ""      # full API tag query
    pool: Pool | None = None


class MainWindow(QMainWindow):
    fetch_status = Signal(str)  # emitted from worker threads

    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("E621 Cruiser")
        assets = Path(__file__).resolve().parent.parent / "assets"
        icon_path = (assets / "e621-cruiser.ico") if (assets / "e621-cruiser.ico").exists() else (assets / "e621-cruiser.png")
        if icon_path.exists():
            self.setWindowIcon(QIcon(str(icon_path)))
        self.resize(1400, 900)
        self.settings = Settings.load()
        self.history = History(DATA_DIR / "history.db")
        self.api = Runner(2, self)      # API calls (rate-limited inside the client)
        self.tag_api = Runner(1, self)  # Dedicated runner for tag autocomplete
        self.thumbs = Runner(6, self)   # thumbnails / previews from the static CDN
        self.downloads = DownloadManager(self.history, self)
        self.client = E621Client()
        self.blacklist = Blacklist()

        self._gen = 0
        self._page = 1
        self._exhausted = True
        self._loading = False
        self._hidden = 0
        self._source = Source("search", "")
        self._stop_fetch = False
        self._preview_dlg: PreviewDialog | None = None

        self._build_ui()
        self._load_settings_into_ui()
        self._apply_settings()

        self.fetch_status.connect(self.statusBar().showMessage)
        self.downloads.progress.connect(self._on_dl_progress)
        self.downloads.item_finished.connect(self._on_dl_item)
        self.downloads.idle.connect(lambda: self._log("Downloads finished."))

        if self.settings.window_geometry:
            self.restoreGeometry(QByteArray.fromBase64(self.settings.window_geometry.encode()))

        QTimer.singleShot(100, self._initial_connect)

    # =============================================================== UI setup
    def _build_ui(self) -> None:
        # ---- menu
        m = self.menuBar().addMenu("&File")
        act = QAction("&Settings…", self, shortcut=QKeySequence("Ctrl+,"))
        act.triggered.connect(self._open_settings)
        m.addAction(act)
        act = QAction("&Test Connection", self)
        act.triggered.connect(self._manual_test_connection)
        m.addAction(act)
        act = QAction("&Open download folder", self)
        act.triggered.connect(self._open_folder)
        m.addAction(act)
        m.addSeparator()
        act = QAction("&Quit", self, shortcut=QKeySequence.StandardKey.Quit)
        act.triggered.connect(self.close)
        m.addAction(act)

        v = self.menuBar().addMenu("&View")
        act_inspector = QAction("&Tag Inspector", self, checkable=True)
        act_inspector.setChecked(True)
        act_inspector.toggled.connect(lambda visible: self.inspector.setVisible(visible))
        v.addAction(act_inspector)

        # ---- left: sources
        self.tabs = QTabWidget()
        self.tabs.addTab(self._build_search_tab(), "Search")
        self.tabs.addTab(self._build_pools_tab(), "Pools")
        self.tabs.addTab(self._build_favs_tab(), "Favorites")
        self.tabs.setMinimumWidth(260)

        # ---- center: grid
        self.grid = PostGrid()
        self.grid.near_bottom.connect(self._load_next_page)
        self.grid.post_clicked.connect(self._preview)
        self.grid.itemDoubleClicked.connect(lambda it: self._preview(self.grid.post_for(it)))
        self.grid.itemSelectionChanged.connect(self._update_selection_label)
        self.grid.currentItemChanged.connect(self._on_grid_current_changed)
        self.grid.customContextMenuRequested.connect(self._grid_menu)
        self.result_label = QLabel("Search for tags or open a pool to get started.")

        # ---- bottom: download panel
        dl_box = QGroupBox("Download")
        self.dir_edit = QLineEdit()
        browse = QPushButton("Browse…")
        browse.clicked.connect(self._browse_dir)
        open_btn = QPushButton("Open")
        open_btn.clicked.connect(self._open_folder)
        self.organize = QComboBox()
        for key, label in ORGANIZE_MODES:
            self.organize.addItem(label, key)
        self.organize.currentIndexChanged.connect(self._organize_changed)
        self.tags_file_cb = QCheckBox("Save tags .txt")
        self.tags_file_cb.toggled.connect(lambda v: setattr(self.settings, "write_tags_file", v))

        sel_all = QPushButton("Select all")
        sel_all.clicked.connect(self.grid.selectAll)
        sel_none = QPushButton("Select none")
        sel_none.clicked.connect(self.grid.clearSelection)
        self.dl_sel_btn = QPushButton("⬇ Download selected (0)")
        self.dl_sel_btn.setShortcut(QKeySequence("Ctrl+D"))
        self.dl_sel_btn.clicked.connect(self._download_selected)
        self.dl_all_btn = QPushButton("⬇ Download ALL results")
        self.dl_all_btn.clicked.connect(self._download_all)
        cancel = QPushButton("Cancel downloads")
        cancel.clicked.connect(self._cancel_downloads)
        for b in (self.dl_sel_btn, self.dl_all_btn):
            b.setStyleSheet("font-weight: bold; padding: 6px 12px;")

        self.progress = QProgressBar()
        self.progress.setFormat("Idle")
        self.progress.setValue(0)
        self.progress.setMaximum(1)
        self.log_view = QPlainTextEdit(readOnly=True)
        self.log_view.setMaximumBlockCount(1000)
        self.log_view.setMaximumHeight(110)

        row1 = QHBoxLayout()
        row1.addWidget(QLabel("Folder:"))
        row1.addWidget(self.dir_edit, 1)
        row1.addWidget(browse)
        row1.addWidget(open_btn)
        row1.addSpacing(16)
        row1.addWidget(QLabel("Organize:"))
        row1.addWidget(self.organize)
        row1.addWidget(self.tags_file_cb)
        row2 = QHBoxLayout()
        row2.addWidget(sel_all)
        row2.addWidget(sel_none)
        row2.addStretch()
        row2.addWidget(self.dl_sel_btn)
        row2.addWidget(self.dl_all_btn)
        row2.addWidget(cancel)
        dl_lay = QVBoxLayout(dl_box)
        dl_lay.addLayout(row1)
        dl_lay.addLayout(row2)
        dl_lay.addWidget(self.progress)
        dl_lay.addWidget(self.log_view)

        center_widget = QWidget()
        rl = QVBoxLayout(center_widget)
        rl.setContentsMargins(0, 0, 0, 0)
        rl.addWidget(self.result_label)
        rl.addWidget(self.grid, 1)
        rl.addWidget(dl_box)

        # ---- right: tag inspector
        self.inspector = PostInspector()
        self.inspector.tag_clicked.connect(self._toggle_tag_in_search)
        self.inspector.tag_exclude_requested.connect(self._exclude_tag_in_search)
        self.inspector.tag_search_requested.connect(self._search_only_tag)
        self.inspector.preview_requested.connect(self._preview)
        self.inspector.download_requested.connect(lambda p: self._enqueue([p]))
        self.inspector.setMinimumWidth(260)

        split = QSplitter()
        split.addWidget(self.tabs)
        split.addWidget(center_widget)
        split.addWidget(self.inspector)
        split.setStretchFactor(0, 0)
        split.setStretchFactor(1, 1)
        split.setStretchFactor(2, 0)
        split.setSizes([260, 860, 280])
        self.setCentralWidget(split)

        self.auth_label = QLabel()
        self.statusBar().addPermanentWidget(self.auth_label)

    def _build_search_tab(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        self.tag_edit = TagLineEdit()
        self.tag_edit.returnPressed.connect(self._start_search)
        self.tag_edit.suggestions_wanted.connect(self._autocomplete)
        self.tag_edit.textChanged.connect(lambda _t: self._refresh_tag_views())
        find = QAction(self)
        find.setShortcut(QKeySequence.StandardKey.Find)
        find.triggered.connect(lambda: (self.tabs.setCurrentIndex(0), self.tag_edit.setFocus()))
        self.addAction(find)

        hint = QLabel("<small>Space-separated tags. <b>-tag</b> excludes, <b>~tag</b> = OR. "
                      "Metatags like <b>score:&gt;100</b> and <b>artist:name</b> work too.</small>")
        hint.setWordWrap(True)

        self.cb_s = QCheckBox("Safe")
        self.cb_q = QCheckBox("Questionable")
        self.cb_e = QCheckBox("Explicit")
        rating_row = QHBoxLayout()
        for cb in (self.cb_s, self.cb_q, self.cb_e):
            rating_row.addWidget(cb)

        self.min_score = QSpinBox()
        self.min_score.setRange(MIN_SCORE_ANY, 100000)
        self.min_score.setSpecialValueText("Any")
        self.min_score.setSingleStep(10)
        self.order = QComboBox()
        for label, val in ORDERS:
            self.order.addItem(label, val)

        form = QFormLayout()
        form.addRow("Rating:", rating_row)
        form.addRow("Min score:", self.min_score)
        form.addRow("Sort:", self.order)

        go = QPushButton("🔍 Search")
        go.setStyleSheet("font-weight: bold; padding: 6px;")
        go.clicked.connect(lambda: self._start_search())

        saved_box = QGroupBox("Saved searches (double-click to run)")
        self.saved_list = QListWidget()
        self.saved_list.itemDoubleClicked.connect(self._run_saved)
        save_btn = QPushButton("Save current")
        save_btn.clicked.connect(self._save_search)
        del_btn = QPushButton("Remove")
        del_btn.clicked.connect(self._remove_saved)
        sl = QVBoxLayout(saved_box)
        sl.addWidget(self.saved_list)
        br = QHBoxLayout()
        br.addWidget(save_btn)
        br.addWidget(del_btn)
        sl.addLayout(br)

        lay.addWidget(QLabel("Tags:"))
        lay.addWidget(self.tag_edit)
        lay.addWidget(hint)
        lay.addLayout(form)
        lay.addWidget(go)
        lay.addWidget(saved_box, 1)
        return w

    def _build_pools_tab(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        self.pool_edit = QLineEdit()
        self.pool_edit.setPlaceholderText("Pool name or ID")
        self.pool_edit.returnPressed.connect(self._search_pools)
        go = QPushButton("Find pools")
        go.clicked.connect(self._search_pools)
        self.pool_list = QListWidget()
        self.pool_list.itemDoubleClicked.connect(lambda it: self._open_pool(it.data(Qt.ItemDataRole.UserRole)))
        open_btn = QPushButton("Show pool")
        open_btn.clicked.connect(self._open_selected_pool)
        dl_btn = QPushButton("⬇ Download whole pool")
        dl_btn.setStyleSheet("font-weight: bold;")
        dl_btn.clicked.connect(self._download_selected_pool)
        row = QHBoxLayout()
        row.addWidget(self.pool_edit, 1)
        row.addWidget(go)
        lay.addLayout(row)
        lay.addWidget(self.pool_list, 1)
        r2 = QHBoxLayout()
        r2.addWidget(open_btn)
        r2.addWidget(dl_btn)
        lay.addLayout(r2)
        tip = QLabel("<small>Pools are saved to <b>pools/&lt;name&gt; (id)/</b> with pages "
                     "numbered in reading order (0001_…, 0002_…).</small>")
        tip.setWordWrap(True)
        lay.addWidget(tip)
        return w

    def _build_favs_tab(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        self.fav_user = QLineEdit()
        self.fav_user.setPlaceholderText("username (defaults to your account)")
        self.fav_user.returnPressed.connect(self._load_favorites)
        go = QPushButton("Load favorites")
        go.clicked.connect(self._load_favorites)
        note = QLabel("<small>Uses the rating / score / sort filters from the Search tab. "
                      "Private favorites require your own API key.</small>")
        note.setWordWrap(True)
        lay.addWidget(QLabel("User:"))
        lay.addWidget(self.fav_user)
        lay.addWidget(go)
        lay.addWidget(note)
        lay.addStretch()
        return w

    # ============================================================== settings
    def _load_settings_into_ui(self) -> None:
        s = self.settings
        self.dir_edit.setText(s.download_dir)
        idx = self.organize.findData(s.organize_mode)
        self.organize.setCurrentIndex(max(0, idx))
        self.tags_file_cb.setChecked(s.write_tags_file)
        self.cb_s.setChecked(s.rating_s)
        self.cb_q.setChecked(s.rating_q)
        self.cb_e.setChecked(s.rating_e)
        self.min_score.setValue(s.min_score)
        self.order.setCurrentIndex(max(0, self.order.findData(s.order)))
        self.fav_user.setText(s.favorites_user)
        self.saved_list.clear()
        self.saved_list.addItems(s.saved_searches)

    def _save_ui_to_settings(self) -> None:
        s = self.settings
        s.download_dir = self.dir_edit.text().strip() or s.download_dir
        s.organize_mode = self.organize.currentData()
        s.write_tags_file = self.tags_file_cb.isChecked()
        s.rating_s, s.rating_q, s.rating_e = (self.cb_s.isChecked(), self.cb_q.isChecked(),
                                              self.cb_e.isChecked())
        s.min_score = self.min_score.value()
        s.order = self.order.currentData()
        s.favorites_user = self.fav_user.text().strip()
        s.saved_searches = [self.saved_list.item(i).text() for i in range(self.saved_list.count())]
        s.window_geometry = bytes(self.saveGeometry().toBase64()).decode()

    def _apply_settings(self) -> None:
        s = self.settings
        self.client = E621Client(s.username, s.api_key)
        self.blacklist = Blacklist(s.blacklist)
        self.downloads.configure(self.client, s.concurrent_downloads)
        self.tags_file_cb.setChecked(s.write_tags_file)
        if self.client.authenticated:
            self.auth_label.setText(f"Logged in as {s.username}")
        else:
            self.auth_label.setText("Anonymous (add API key in Settings)")

    def _manual_test_connection(self) -> None:
        self.statusBar().showMessage("Testing connection to e621…")
        def check():
            return self.client.check_connection()
        def on_checked(res):
            ok, msg = res
            if ok:
                self.statusBar().showMessage(f"✓ {msg}", 6000)
                QMessageBox.information(self, "Connection Successful", msg)
            else:
                self.statusBar().showMessage(f"✗ {msg}", 8000)
                QMessageBox.warning(self, "Connection Failed", msg)
        self.api.submit(check, on_result=on_checked)

    def _initial_connect(self) -> None:
        def check():
            return self.client.check_connection()
        def on_checked(res):
            ok, msg = res
            if ok:
                self.statusBar().showMessage(f"✓ {msg}", 5000)
                self.auth_label.setText(f"✓ {self.settings.username or 'Anonymous'}")
            else:
                self.statusBar().showMessage(f"✗ {msg}", 8000)
                self.auth_label.setText("⚠ Connection error")
        self.api.submit(check, on_result=on_checked)

    def _open_settings(self) -> None:
        self._save_ui_to_settings()
        dlg = SettingsDialog(self.settings, len(self.history), self)
        if dlg.exec():
            dlg.apply_to(self.settings)
            if dlg.clear_history_requested:
                self.history.clear()
            self.settings.save()
            self._apply_settings()
            self._log("Settings saved.")

    def closeEvent(self, e):  # noqa: N802
        if self.downloads.busy:
            r = QMessageBox.question(self, "Downloads running",
                                     "Downloads are still running. Quit anyway?")
            if r != QMessageBox.StandardButton.Yes:
                e.ignore()
                return
        self._stop_fetch = True
        self.api.stop()
        self.tag_api.stop()
        self.thumbs.stop()
        self.downloads.cancel()
        self._save_ui_to_settings()
        self.settings.save()
        e.accept()
        QApplication.instance().quit()

    # ================================================================ search
    def _compose_query(self, base: str) -> str:
        clean_tokens = [tok.strip() for tok in base.split() if tok.strip()]
        parts = [" ".join(clean_tokens)] if clean_tokens else []
        picked = [r for r, cb in (("s", self.cb_s), ("q", self.cb_q), ("e", self.cb_e))
                  if cb.isChecked()]
        if len(picked) == 1:
            parts.append(f"rating:{picked[0]}")
        elif len(picked) == 2:
            parts.append(f"-rating:{({'s', 'q', 'e'} - set(picked)).pop()}")
        if self.min_score.value() > MIN_SCORE_ANY:
            parts.append(f"score:>={self.min_score.value()}")
        if self.order.currentData() and "order:" not in base:
            parts.append(self.order.currentData())
        return " ".join(parts).strip()

    def _autocomplete(self, token: str) -> None:
        self.tag_api.submit(self.client.autocomplete, token,
                            on_result=lambda tags: self.tag_edit.set_suggestions(token, tags),
                            on_error=lambda e: None)

    def _start_search(self, raw: str | bool | None = None, label: str | None = None,
                      kind: str = "search") -> None:
        if raw is None or isinstance(raw, bool):
            raw = self.tag_edit.text().strip()
        else:
            raw = str(raw).strip()
        if not any((self.cb_s.isChecked(), self.cb_q.isChecked(), self.cb_e.isChecked())):
            QMessageBox.warning(self, "No rating", "Select at least one rating.")
            return
        self._begin(Source(kind, label or raw or "latest", self._compose_query(raw)))
        self._exhausted = False
        self._load_next_page()

    def _begin(self, source: Source) -> None:
        self._gen += 1
        self._stop_fetch = True
        self.thumbs.clear_pending()
        self.grid.clear_posts()
        self.inspector.set_post(None)
        self._source = source
        self._page = 1
        self._hidden = 0
        self._loading = False
        self._exhausted = True
        self._update_result_label()

    def _load_next_page(self) -> None:
        if self._loading or self._exhausted or self._source.kind == "pool":
            return
        self._loading = True
        self.statusBar().showMessage(f"Loading page {self._page}…")
        self.api.submit(self.client.search_posts, self._source.query, self._page,
                        on_result=partial(self._on_page, self._gen),
                        on_error=partial(self._on_api_error, self._gen))

    def _on_page(self, gen: int, posts: list[Post]) -> None:
        if gen != self._gen:
            return
        self._loading = False
        if len(posts) < POSTS_PER_PAGE or self._page >= 750:
            self._exhausted = True
        self._page += 1
        self._show_posts(posts)
        self.statusBar().showMessage("Ready", 3000)
        # keep filling until the grid scrolls (e.g. when a lot was blacklisted)
        if not self._exhausted and not self.grid.is_scrollable():
            QTimer.singleShot(50, self._load_next_page)

    def _show_posts(self, posts: list[Post]) -> None:
        kept, hidden = self.blacklist.filter(posts)
        for p in kept:
            self.tag_edit.learn_post_tags(p)
        self._hidden += hidden
        added = self.grid.add_posts(kept, {p.id for p in kept if self.history.has(p.id)})
        gen = self._gen
        for p in added:
            if p.preview_url:
                self.thumbs.submit(fetch_image, p.preview_url,
                                   on_result=partial(self._on_thumb, gen, p.id),
                                   on_error=lambda e: None)
        self._update_result_label()

    def _on_thumb(self, gen: int, post_id: int, img: QImage) -> None:
        if gen == self._gen:
            self.grid.set_thumbnail(post_id, img)
            self.inspector.set_thumbnail(post_id, img)

    def _on_api_error(self, gen: int, err: Exception) -> None:
        if gen != self._gen:
            return
        self._loading = False
        self._exhausted = True
        self.statusBar().showMessage(f"Error: {err}")
        self._log(f"Error: {err}")
        self.result_label.setText(f"<span style='color: #e06c75;'>⚠ Error loading results: {err}</span>")

    def _update_result_label(self) -> None:
        s = self._source
        if s.kind == "pool" and s.pool:
            head = f"Pool: <b>{s.pool.display_name}</b> ({s.pool.post_count} posts)"
        elif s.kind == "favorites":
            head = f"Favorites of <b>{s.label.removeprefix('favorites_')}</b>"
        else:
            head = f"Search: <b>{s.query or '(latest posts)'}</b>"
        more = "" if self._exhausted else " (scroll for more)"
        hidden = f" · {self._hidden} hidden by blacklist" if self._hidden else ""
        self.result_label.setText(f"{head} · {self.grid.count()} shown{more}{hidden}")

    def _update_selection_label(self) -> None:
        self.dl_sel_btn.setText(f"⬇ Download selected ({len(self.grid.selectedItems())})")

    # --------------------------------------------------------- saved searches
    def _save_search(self) -> None:
        q = self.tag_edit.text().strip()
        existing = [self.saved_list.item(i).text() for i in range(self.saved_list.count())]
        if q and q not in existing:
            self.saved_list.addItem(q)
            self._save_ui_to_settings()
            self.settings.save()

    def _remove_saved(self) -> None:
        for it in self.saved_list.selectedItems():
            self.saved_list.takeItem(self.saved_list.row(it))
        self._save_ui_to_settings()
        self.settings.save()

    def _run_saved(self, item: QListWidgetItem) -> None:
        self.tag_edit.setText(item.text())
        self._start_search()

    # ================================================================= pools
    def _search_pools(self) -> None:
        q = self.pool_edit.text().strip()
        if not q:
            return
        self.pool_list.clear()
        self.statusBar().showMessage("Searching pools…")
        self.api.submit(self.client.search_pools, q,
                        on_result=self._on_pools, on_error=partial(self._on_api_error, self._gen))

    def _on_pools(self, pools: list[Pool]) -> None:
        self.pool_list.clear()
        for p in pools:
            it = QListWidgetItem(f"{p.display_name}  [{p.post_count}]")
            it.setData(Qt.ItemDataRole.UserRole, p)
            it.setToolTip(f"Pool #{p.id} · {p.category} · {p.post_count} posts")
            self.pool_list.addItem(it)
        self.statusBar().showMessage(f"{len(pools)} pool(s) found", 4000)

    def _selected_pool(self) -> Pool | None:
        it = self.pool_list.currentItem()
        return it.data(Qt.ItemDataRole.UserRole) if it else None

    def _open_selected_pool(self) -> None:
        if pool := self._selected_pool():
            self._open_pool(pool)

    def _open_pool(self, pool: Pool) -> None:
        self._begin(Source("pool", pool.display_name, f"pool:{pool.id}", pool))
        self.statusBar().showMessage(f"Loading pool {pool.display_name}…")
        self.api.submit(self.client.get_pool_posts, pool,
                        on_result=partial(self._on_pool_posts, self._gen),
                        on_error=partial(self._on_api_error, self._gen))

    def _on_pool_posts(self, gen: int, posts: list[Post]) -> None:
        if gen != self._gen:
            return
        self._show_posts(posts)
        self.statusBar().showMessage("Pool loaded", 3000)

    def _open_pool_by_id(self, pool_id: int) -> None:
        self.api.submit(self.client.get_pool, pool_id, on_result=self._open_pool,
                        on_error=partial(self._on_api_error, self._gen))

    def _download_selected_pool(self) -> None:
        pool = self._selected_pool()
        if pool is None:
            QMessageBox.information(self, "No pool", "Select a pool in the list first.")
            return
        if self._source.kind == "pool" and self._source.pool and self._source.pool.id == pool.id \
                and self.grid.count():
            self._enqueue(self.grid.all_posts(), self._source)
            return
        self.statusBar().showMessage(f"Fetching pool {pool.display_name}…")
        src = Source("pool", pool.display_name, f"pool:{pool.id}", pool)
        self.api.submit(self.client.get_pool_posts, pool,
                        on_result=lambda posts: self._enqueue(self.blacklist.filter(posts)[0], src),
                        on_error=partial(self._on_api_error, self._gen))

    # ============================================================= favorites
    def _load_favorites(self) -> None:
        user = self.fav_user.text().strip() or self.settings.username
        if not user:
            QMessageBox.information(self, "No user",
                                    "Enter a username or set your account in Settings.")
            return
        self._start_search(f"fav:{user}", label=f"favorites_{user}", kind="favorites")

    # ============================================================= downloads
    def _make_jobs(self, posts: list[Post], src: Source) -> list[DownloadJob]:
        s = self.settings
        base = self.dir_edit.text().strip() or s.download_dir
        mode = self.organize.currentData()
        idx = {pid: i + 1 for i, pid in enumerate(src.pool.post_ids)} if src.pool else {}
        jobs = []
        for p in posts:
            j = DownloadJob(post=p, base_dir=base, mode=mode, source=src.label,
                            write_tags=self.tags_file_cb.isChecked(),
                            skip_downloaded=s.skip_downloaded)
            if src.pool:
                j.pool_id, j.pool_name, j.pool_index = src.pool.id, src.pool.display_name, idx.get(p.id)
            jobs.append(j)
        return jobs

    def _enqueue(self, posts: list[Post], src: Source | None = None) -> None:
        if not posts:
            self.statusBar().showMessage("Nothing to download.", 4000)
            return
        jobs = self._make_jobs(posts, src or self._source)
        self.downloads.enqueue(jobs)
        self._log(f"Queued {len(jobs)} post(s) → {jobs[0].base_dir} [{self.organize.currentText()}]")

    def _download_selected(self) -> None:
        posts = self.grid.selected_posts()
        if not posts:
            QMessageBox.information(self, "Nothing selected",
                                    "Select thumbnails first (Ctrl/Shift-click, or Select all).")
            return
        self._enqueue(posts)

    def _download_all(self) -> None:
        if self._exhausted or self._source.kind == "pool":
            self._enqueue(self.grid.all_posts())
            return
        # Fetch the remaining pages in the background, then confirm.
        src, start, gen = self._source, self._page, self._gen
        shown = self.grid.all_posts()
        self._stop_fetch = False

        def work() -> list[Post]:
            return self.client.fetch_all_posts(
                src.query, start,
                progress=lambda pg, n: self.fetch_status.emit(
                    f"Collecting results… page {pg}, {len(shown) + n} posts"),
                should_stop=lambda: self._stop_fetch)

        def done(more: list[Post]) -> None:
            if gen != self._gen:
                return
            kept, hidden = self.blacklist.filter(more)
            seen = {p.id for p in shown}
            everything = shown + [p for p in kept if p.id not in seen]
            new = sum(1 for p in everything if not self.history.has(p.id))
            r = QMessageBox.question(
                self, "Download all results",
                f"Found {len(everything):,} posts for “{src.query or 'latest'}”"
                f" ({new:,} not downloaded yet"
                + (f", {hidden:,} more hidden by blacklist" if hidden else "")
                + ").\n\nDownload them all?")
            if r == QMessageBox.StandardButton.Yes:
                self._enqueue(everything, src)
            self.statusBar().showMessage("Ready", 3000)

        self.statusBar().showMessage("Collecting all results…")
        self.api.submit(work, on_result=done, on_error=partial(self._on_api_error, gen))

    def _cancel_downloads(self) -> None:
        self._stop_fetch = True
        self.downloads.cancel()
        self._log("Downloads cancelled.")

    def _on_dl_progress(self, done: int, total: int) -> None:
        self.progress.setMaximum(max(total, 1))
        self.progress.setValue(done)
        self.progress.setFormat("%v / %m downloaded" if total else "Idle")

    def _on_dl_item(self, post_id: int, status: str, detail: str) -> None:
        if status in ("ok", "skipped"):
            self.grid.mark_downloaded(post_id)
        if status == "ok":
            self._log(f"✓ #{post_id} → {detail}")
        elif status == "error":
            self._log(f"✗ #{post_id}: {detail}")
        elif status == "skipped":
            self._log(f"• #{post_id} skipped ({detail})")

    def _log(self, msg: str) -> None:
        self.log_view.appendPlainText(msg)

    # ================================================================== misc
    def _organize_changed(self) -> None:
        self.settings.organize_mode = self.organize.currentData()

    def _browse_dir(self) -> None:
        d = QFileDialog.getExistingDirectory(self, "Download folder", self.dir_edit.text())
        if d:
            self.dir_edit.setText(d)
            self.settings.download_dir = d

    def _open_folder(self) -> None:
        path = Path(self.dir_edit.text().strip() or self.settings.download_dir).expanduser()
        path.mkdir(parents=True, exist_ok=True)
        QDesktopServices.openUrl(QUrl.fromLocalFile(str(path)))

    def _on_grid_current_changed(self, current: QListWidgetItem | None, _prev: QListWidgetItem | None = None) -> None:
        if current is None:
            self.inspector.set_post(None)
            return
        post = self.grid.post_for(current)
        img = self.grid._images.get(post.id)
        self.inspector.set_post(post, img, self.tag_edit.text())

    def _preview(self, post: Post) -> None:
        all_posts = self.grid.all_posts()
        idx = all_posts.index(post) if post in all_posts else 0
        dlg = PreviewDialog(post, all_posts=all_posts, index=idx,
                            active_query=self.tag_edit.text(), parent=self)
        self._preview_dlg = dlg
        dlg.add_tag.connect(self._toggle_tag_in_search)
        dlg.exclude_tag.connect(self._exclude_tag_in_search)
        dlg.search_tag.connect(self._search_only_tag)
        dlg.download.connect(lambda p: self._enqueue([p]))
        dlg.post_changed.connect(lambda p: self._load_preview_image(dlg, p))
        self._load_preview_image(dlg, post)
        dlg.showMaximized()

    def _load_preview_image(self, dlg: PreviewDialog, post: Post) -> None:
        if thumb := self.grid._images.get(post.id):
            dlg.set_image(thumb)
        url = post.sample_url or post.file_url or post.preview_url
        if url:
            self.thumbs.submit(
                fetch_image, url,
                on_result=lambda img, p=post: dlg.set_image(img) if dlg.post.id == p.id else None,
                on_error=lambda e, p=post: dlg.set_error(str(e)) if dlg.post.id == p.id else None,
            )
        else:
            dlg.set_error("No image URL (post may require login in Settings).")

    def _toggle_tag_in_search(self, tag: str) -> None:
        self.tabs.setCurrentIndex(0)
        tokens = self.tag_edit.text().split()
        if tag in tokens:
            tokens.remove(tag)
            self.statusBar().showMessage(f"Removed “{tag}” from search", 3000)
        elif f"-{tag}" in tokens:
            tokens.remove(f"-{tag}")
            tokens.append(tag)
            self.statusBar().showMessage(f"Added “{tag}” to search", 3000)
        else:
            tokens.append(tag)
            self.statusBar().showMessage(f"Added “{tag}” to search", 3000)
        new_text = (" ".join(tokens) + " ") if tokens else ""
        self.tag_edit.setText(new_text)
        self.tag_edit.setCursorPosition(len(new_text))
        self._refresh_tag_views()

    def _exclude_tag_in_search(self, tag: str) -> None:
        self.tabs.setCurrentIndex(0)
        tokens = self.tag_edit.text().split()
        neg = f"-{tag}"
        if neg in tokens:
            tokens.remove(neg)
            self.statusBar().showMessage(f"Removed exclusion “-{tag}”", 3000)
        else:
            if tag in tokens:
                tokens.remove(tag)
            tokens.append(neg)
            self.statusBar().showMessage(f"Excluded “-{tag}” from search", 3000)
        new_text = (" ".join(tokens) + " ") if tokens else ""
        self.tag_edit.setText(new_text)
        self.tag_edit.setCursorPosition(len(new_text))
        self._refresh_tag_views()

    def _search_only_tag(self, tag: str) -> None:
        self.tabs.setCurrentIndex(0)
        self.tag_edit.setText(f"{tag} ")
        self.tag_edit.setCursorPosition(len(tag) + 1)
        self._refresh_tag_views()
        self._start_search()

    def _refresh_tag_views(self) -> None:
        q = self.tag_edit.text()
        self.inspector.refresh_active_tags(q)
        if self._preview_dlg and self._preview_dlg.isVisible():
            self._preview_dlg.refresh_active_tags(q)

    def _add_tag_to_search(self, tag: str) -> None:
        self._toggle_tag_in_search(tag)

    def _grid_menu(self, pos) -> None:
        item = self.grid.itemAt(pos)
        if item is None:
            return
        post = self.grid.post_for(item)
        menu = QMenu(self)
        menu.addAction("Full Screen View", lambda: self._preview(post))
        menu.addAction("Download", lambda: self._enqueue([post]))
        menu.addAction("Open on e621", lambda: QDesktopServices.openUrl(QUrl(post.page_url)))
        if saved := self.history.path_for(post.id):
            if os.path.exists(saved):
                menu.addAction("Show saved file",
                               lambda: QDesktopServices.openUrl(
                                   QUrl.fromLocalFile(str(Path(saved).parent))))
        menu.addSeparator()
        for a in post.artists[:3]:
            menu.addAction(f"Add artist “{a}” to search",
                           lambda a=a: self._toggle_tag_in_search(a))
            menu.addAction(f"Search ONLY artist “{a}”",
                           lambda a=a: self._search_only_tag(a))
        for pid in post.pools[:3]:
            menu.addAction(f"Open pool #{pid}", lambda pid=pid: self._open_pool_by_id(pid))
        menu.exec(self.grid.viewport().mapToGlobal(pos))
