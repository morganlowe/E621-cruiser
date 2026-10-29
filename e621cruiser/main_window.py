"""Main application window."""
from __future__ import annotations

import html
import os
from dataclasses import dataclass
from functools import partial
from pathlib import Path

from PySide6.QtCore import QByteArray, Qt, QTimer, QUrl, Signal
from PySide6.QtGui import QAction, QDesktopServices, QIcon, QImage, QKeySequence
from PySide6.QtWidgets import (QApplication, QCheckBox, QComboBox, QFileDialog, QFormLayout,
                               QFrame, QGridLayout, QGroupBox, QHBoxLayout, QLabel, QLineEdit,
                               QListWidget, QListWidgetItem, QMainWindow, QMenu, QMessageBox,
                               QPlainTextEdit, QProgressBar, QPushButton, QScrollArea, QSpinBox,
                               QSplitter, QTabWidget, QVBoxLayout, QWidget)

from .api import POSTS_PER_PAGE, E621Client, Pool, Post
from .blacklist import Blacklist
from .downloader import DownloadManager
from .settings import (DATA_DIR, MEDIA_FILTERS, MIN_SCORE_ANY, QUICK_BLACKLIST_ITEMS,
                       Settings, full_blacklist)
from .storage import ORGANIZE_MODES, DownloadJob, History, SpankBank
from .widgets import (CollapsibleSection, PostGrid, PostInspector, PreviewDialog,
                      SettingsDialog, TagLineEdit, TagListWidget, artist_page_url)
from .workers import Runner, fetch_image

ORDERS = [("Newest", ""), ("Highest score", "order:score"), ("Most favorited", "order:favcount"),
          ("Random", "order:random"), ("Oldest", "order:id")]


@dataclass
class Source:
    kind: str            # "search" | "pool" | "favorites" | "artist" | "spank_bank"
    label: str           # used for folder names in "By search" mode
    query: str = ""      # full API tag query
    pool: Pool | None = None


@dataclass
class TabSnapshot:
    source: Source
    posts: list[Post]
    page: int
    exhausted: bool
    hidden: int
    scroll_pos: int


class ResultTab(QWidget):
    back_requested = Signal()
    forward_requested = Signal()
    download_pool_requested = Signal()
    slideshow_requested = Signal()
    bank_pool_requested = Signal()

    def __init__(self, source: Source | None = None, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.source = source or Source("search", "latest", "")
        self.page = 1
        self.gen = 0
        self.exhausted = True
        self.loading = False
        self.hidden = 0
        self.history_back: list[TabSnapshot] = []
        self.history_forward: list[TabSnapshot] = []

        lay = QVBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.setSpacing(2)

        nav_bar = QHBoxLayout()
        nav_bar.setContentsMargins(6, 4, 6, 2)
        nav_bar.setSpacing(6)

        self.btn_back = QPushButton("< Back")
        self.btn_back.setToolTip("Go back to previous view (Alt+Left)")
        self.btn_back.setEnabled(False)
        self.btn_back.setStyleSheet("padding: 4px 10px; font-weight: bold;")
        self.btn_back.clicked.connect(self.back_requested)

        self.btn_forward = QPushButton("Forward >")
        self.btn_forward.setToolTip("Go forward (Alt+Right)")
        self.btn_forward.setEnabled(False)
        self.btn_forward.setStyleSheet("padding: 4px 10px; font-weight: bold;")
        self.btn_forward.clicked.connect(self.forward_requested)

        self.result_label = QLabel("Search for tags or open a pool to get started.")
        self.result_label.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse
                                                  | Qt.TextInteractionFlag.LinksAccessibleByMouse)
        self.result_label.setOpenExternalLinks(True)
        self.result_label.setStyleSheet("padding: 0 4px; font-size: 13px; color: #d7dae0;")

        self.btn_slideshow = QPushButton("Slideshow")
        self.btn_slideshow.setToolTip("Play full screen slideshow of current posts (Space)")
        self.btn_slideshow.setStyleSheet("""
            QPushButton {
                font-weight: bold;
                padding: 4px 12px;
                background-color: #1e3a8a;
                color: #ffffff;
                border-radius: 4px;
            }
            QPushButton:hover {
                background-color: #2563eb;
            }
        """)
        self.btn_slideshow.clicked.connect(self.slideshow_requested)

        self.btn_bank_pool = QPushButton("+ Bank")
        self.btn_bank_pool.setToolTip("Save this whole pool to Spank Bank for later online viewing")
        self.btn_bank_pool.setStyleSheet("""
            QPushButton {
                font-weight: bold;
                padding: 4px 12px;
                background-color: #374151;
                color: #fbbf24;
                border-radius: 4px;
            }
            QPushButton:hover {
                background-color: #4b5563;
            }
        """)
        self.btn_bank_pool.setVisible(False)
        self.btn_bank_pool.clicked.connect(self.bank_pool_requested)

        self.btn_dl_pool = QPushButton("Download Pool")
        self.btn_dl_pool.setToolTip("Download all posts in this pool")
        self.btn_dl_pool.setStyleSheet("""
            QPushButton {
                font-weight: bold;
                padding: 4px 14px;
                background-color: #2e7d32;
                color: #ffffff;
                border-radius: 4px;
            }
            QPushButton:hover {
                background-color: #388e3c;
            }
        """)
        self.btn_dl_pool.setVisible(False)
        self.btn_dl_pool.clicked.connect(self.download_pool_requested)

        nav_bar.addWidget(self.btn_back)
        nav_bar.addWidget(self.btn_forward)
        nav_bar.addWidget(self.result_label, 1)
        nav_bar.addWidget(self.btn_slideshow)
        nav_bar.addWidget(self.btn_bank_pool)
        nav_bar.addWidget(self.btn_dl_pool)

        self.grid = PostGrid()

        lay.addLayout(nav_bar)
        lay.addWidget(self.grid, 1)

    def update_nav_buttons(self, in_bank: bool = False) -> None:
        """Refresh nav buttons. ``in_bank`` refers to the current pool or artist source."""
        self.btn_back.setEnabled(bool(self.history_back))
        self.btn_forward.setEnabled(bool(self.history_forward))
        is_pool = self.source.kind == "pool"
        is_artist = self.source.kind == "artist"
        self.btn_bank_pool.setVisible(is_pool or is_artist)
        if is_pool or is_artist:
            what = "pool" if is_pool else "artist"
            if in_bank:
                self.btn_bank_pool.setText("In Bank" if is_pool else "Artist In Bank")
                self.btn_bank_pool.setStyleSheet("""
                    QPushButton {
                        font-weight: bold;
                        padding: 4px 12px;
                        background-color: #d97706;
                        color: #ffffff;
                        border-radius: 4px;
                    }
                    QPushButton:hover {
                        background-color: #b45309;
                    }
                """)
                self.btn_bank_pool.setToolTip(f"This {what} is saved in Spank Bank (click to remove)")
            else:
                self.btn_bank_pool.setText("+ Bank" if is_pool else "+ Bank Artist")
                self.btn_bank_pool.setStyleSheet("""
                    QPushButton {
                        font-weight: bold;
                        padding: 4px 12px;
                        background-color: #374151;
                        color: #fbbf24;
                        border-radius: 4px;
                    }
                    QPushButton:hover {
                        background-color: #4b5563;
                    }
                """)
                self.btn_bank_pool.setToolTip("Save this whole pool to Spank Bank for later online viewing"
                                              if is_pool else "Save this artist to Spank Bank")
        self.btn_dl_pool.setVisible(is_pool)
        if is_pool and self.source.pool:
            self.btn_dl_pool.setText(f"Download Pool ({self.source.pool.post_count} posts)")
        else:
            self.btn_dl_pool.setText("Download Pool")


class MainWindow(QMainWindow):
    fetch_status = Signal(str)  # emitted from worker threads

    def __init__(self, settings: Settings | None = None, data_dir: Path | None = None) -> None:
        super().__init__()
        self.setWindowTitle("E621 Cruiser")
        assets = Path(__file__).resolve().parent.parent / "assets"
        icon_path = (assets / "e621-cruiser.ico") if (assets / "e621-cruiser.ico").exists() else (assets / "e621-cruiser.png")
        if icon_path.exists():
            self.setWindowIcon(QIcon(str(icon_path)))
        self.resize(1400, 900)
        self.settings = settings if settings is not None else Settings.load()
        data_dir = Path(data_dir) if data_dir is not None else DATA_DIR
        self.history = History(data_dir / "history.db")
        self.spank_bank = SpankBank(data_dir / "spank_bank.db")
        self.api = Runner(2, self)      # API calls (rate-limited inside the client)
        self.tag_api = Runner(1, self)  # Dedicated runner for tag autocomplete
        self.thumbs = Runner(6, self)   # thumbnails / previews from the static CDN
        self.downloads = DownloadManager(self.history, self)
        self.client = E621Client()
        self.blacklist = Blacklist(full_blacklist(self.settings))

        self._stop_fetch = False
        self._preview_dlg: PreviewDialog | None = None
        self._cached_thumbs: dict[int, QImage] = {}

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

    # ============================================================== tab delegation
    @property
    def current_tab(self) -> ResultTab:
        tab = self.results_tabs.currentWidget()
        if not isinstance(tab, ResultTab):
            return self._default_tab
        return tab

    @property
    def grid(self) -> PostGrid:
        return self.current_tab.grid

    @property
    def _source(self) -> Source:
        return self.current_tab.source

    @_source.setter
    def _source(self, val: Source) -> None:
        self.current_tab.source = val

    @property
    def _page(self) -> int:
        return self.current_tab.page

    @_page.setter
    def _page(self, val: int) -> None:
        self.current_tab.page = val

    @property
    def _gen(self) -> int:
        return self.current_tab.gen

    @_gen.setter
    def _gen(self, val: int) -> None:
        self.current_tab.gen = val

    @property
    def _exhausted(self) -> bool:
        return self.current_tab.exhausted

    @_exhausted.setter
    def _exhausted(self, val: bool) -> None:
        self.current_tab.exhausted = val

    @property
    def _loading(self) -> bool:
        return self.current_tab.loading

    @_loading.setter
    def _loading(self, val: bool) -> None:
        self.current_tab.loading = val

    @property
    def _hidden(self) -> int:
        return self.current_tab.hidden

    @_hidden.setter
    def _hidden(self, val: int) -> None:
        self.current_tab.hidden = val

    @property
    def result_label(self) -> QLabel:
        return self.current_tab.result_label

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
        self.tabs.addTab(self._build_spank_bank_tab(), "Spank Bank")
        self.tabs.setMinimumWidth(350)

        # ---- center: tabs of result views
        self.results_tabs = QTabWidget()
        self.results_tabs.setTabsClosable(True)
        self.results_tabs.setMovable(True)
        self.results_tabs.tabCloseRequested.connect(self._close_tab)
        self.results_tabs.currentChanged.connect(self._on_tab_changed)

        new_tab_btn = QPushButton("+")
        new_tab_btn.setToolTip("Open new tab (Ctrl+T)")
        new_tab_btn.setStyleSheet("font-weight: bold; font-size: 14px; padding: 2px 8px;")
        new_tab_btn.clicked.connect(lambda: self._new_empty_tab())
        self.results_tabs.setCornerWidget(new_tab_btn, Qt.Corner.TopRightCorner)

        self._default_tab = self._create_tab(Source("search", "latest", ""), "Search")

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
        sel_all.clicked.connect(lambda: self.grid.selectAll())
        sel_none = QPushButton("Select none")
        sel_none.clicked.connect(lambda: self.grid.clearSelection())
        self.dl_sel_btn = QPushButton("Download selected (0)")
        self.dl_sel_btn.setShortcut(QKeySequence("Ctrl+D"))
        self.dl_sel_btn.clicked.connect(self._download_selected)
        self.dl_all_btn = QPushButton("Download ALL results")
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
        rl.setSpacing(0)
        rl.addWidget(self.results_tabs, 1)
        rl.addWidget(dl_box)

        # ---- right: tag inspector
        self.inspector = PostInspector()
        self.inspector.tag_clicked.connect(self._toggle_tag_in_search)
        self.inspector.tag_exclude_requested.connect(self._exclude_tag_in_search)
        self.inspector.tag_search_requested.connect(self._search_only_tag)
        self.inspector.preview_requested.connect(self._preview)
        self.inspector.download_requested.connect(lambda p: self._enqueue([p]))
        self.inspector.bank_toggle_requested.connect(self._toggle_spank_bank)
        self.inspector.favorite_toggle_requested.connect(self._toggle_favorite)
        self.inspector.upvote_toggle_requested.connect(self._toggle_upvote)
        self.inspector.bank_pool_requested.connect(self._toggle_pool_by_id_in_bank)
        self.inspector.view_pool_requested.connect(lambda pid: self._open_pool_by_id(pid, new_tab=False))
        self.inspector.view_pool_new_tab_requested.connect(lambda pid: self._open_pool_by_id(pid, new_tab=True))
        self.inspector.download_pool_requested.connect(self._download_pool_by_id)
        self.inspector.artist_view_requested.connect(lambda a: self._open_artist(a, new_tab=False))
        self.inspector.artist_view_new_tab_requested.connect(lambda a: self._open_artist(a, new_tab=True))
        self.inspector.artist_bank_toggle_requested.connect(self._toggle_artist_in_bank)
        self.inspector.tag_list.artist_in_bank = self.spank_bank.has_artist
        self.inspector.setMinimumWidth(260)
        self.tabs.setMinimumWidth(350)

        # Global shortcuts for tab navigation
        back_act = QAction(self)
        back_act.setShortcut(QKeySequence("Alt+Left"))
        back_act.triggered.connect(self._navigate_back)
        self.addAction(back_act)

        fwd_act = QAction(self)
        fwd_act.setShortcut(QKeySequence("Alt+Right"))
        fwd_act.triggered.connect(self._navigate_forward)
        self.addAction(fwd_act)

        new_tab_act = QAction(self)
        new_tab_act.setShortcut(QKeySequence("Ctrl+T"))
        new_tab_act.triggered.connect(self._new_empty_tab)
        self.addAction(new_tab_act)

        close_tab_act = QAction(self)
        close_tab_act.setShortcut(QKeySequence("Ctrl+W"))
        close_tab_act.triggered.connect(self._close_current_tab)
        self.addAction(close_tab_act)

        self.split = QSplitter()
        self.split.addWidget(self.tabs)
        self.split.addWidget(center_widget)
        self.split.addWidget(self.inspector)
        self.split.setStretchFactor(0, 0)
        self.split.setStretchFactor(1, 1)
        self.split.setStretchFactor(2, 0)
        if self.settings.splitter_state:
            try:
                self.split.restoreState(QByteArray.fromBase64(self.settings.splitter_state.encode()))
            except Exception:
                self.split.setSizes([360, 760, 280])
        else:
            self.split.setSizes([360, 760, 280])
        self.setCentralWidget(self.split)

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

        self.media_filter = QComboBox()
        for label, val in MEDIA_FILTERS:
            self.media_filter.addItem(label, val)

        self.cb_in_pool = QCheckBox("In pools only")

        self.min_score = QSpinBox()
        self.min_score.setRange(MIN_SCORE_ANY, 100000)
        self.min_score.setSpecialValueText("Any")
        self.min_score.setSingleStep(10)
        self.order = QComboBox()
        for label, val in ORDERS:
            self.order.addItem(label, val)

        form = QFormLayout()
        form.addRow("Rating:", rating_row)
        form.addRow("Media:", self.media_filter)
        form.addRow("Pools:", self.cb_in_pool)
        form.addRow("Min score:", self.min_score)
        form.addRow("Sort:", self.order)

        go = QPushButton("Search")
        go.setStyleSheet("font-weight: bold; padding: 6px;")
        go.clicked.connect(lambda: self._start_search())

        # Quick Blacklist Collapsible Section
        self.bl_section = CollapsibleSection("Quick Blacklist (Exclude)", expanded=self.settings.expand_blacklist)
        self.bl_section.toggled.connect(self._on_blacklist_section_toggled)
        bl_hint = QLabel("<small>One-click exclusions applied to query and local filter.</small>")
        bl_hint.setWordWrap(True)
        self.bl_section.add_widget(bl_hint)

        grid_lay = QGridLayout()
        grid_lay.setHorizontalSpacing(8)
        grid_lay.setVerticalSpacing(4)
        self.quick_bl_checkboxes: dict[str, QCheckBox] = {}
        row, col = 0, 0
        for tag_id, label in QUICK_BLACKLIST_ITEMS:
            cb = QCheckBox(label)
            cb.toggled.connect(self._on_quick_blacklist_toggled)
            self.quick_bl_checkboxes[tag_id] = cb
            grid_lay.addWidget(cb, row, col)
            col += 1
            if col > 1:
                col = 0
                row += 1
        self.bl_section.add_layout(grid_lay)

        btn_row = QHBoxLayout()
        sel_common = QPushButton("Select Common")
        sel_common.setToolTip("Select Gore, Scat, Watersports, Young/Cub, Flash")
        sel_common.clicked.connect(self._select_common_blacklist)
        clear_bl = QPushButton("Clear All")
        clear_bl.clicked.connect(self._clear_quick_blacklist)
        btn_row.addWidget(sel_common)
        btn_row.addWidget(clear_bl)
        self.bl_section.add_layout(btn_row)

        # Saved Searches Collapsible Section
        self.saved_section = CollapsibleSection("Saved Searches", expanded=self.settings.expand_saved)
        self.saved_section.toggled.connect(self._on_saved_section_toggled)
        saved_hint = QLabel("<small>Double-click search below to run.</small>")
        self.saved_section.add_widget(saved_hint)

        self.saved_list = QListWidget()
        self.saved_list.setMaximumHeight(140)
        self.saved_list.itemDoubleClicked.connect(self._run_saved)
        save_btn = QPushButton("Save current")
        save_btn.clicked.connect(self._save_search)
        del_btn = QPushButton("Remove")
        del_btn.clicked.connect(self._remove_saved)
        self.saved_section.add_widget(self.saved_list)
        br = QHBoxLayout()
        br.addWidget(save_btn)
        br.addWidget(del_btn)
        self.saved_section.add_layout(br)

        lay.addWidget(QLabel("Tags:"))
        lay.addWidget(self.tag_edit)
        lay.addWidget(hint)
        lay.addLayout(form)
        lay.addWidget(go)
        lay.addWidget(self.bl_section)
        lay.addWidget(self.saved_section)
        lay.addStretch()

        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        scroll.setWidget(w)
        return scroll

    def _on_blacklist_section_toggled(self, expanded: bool) -> None:
        self.settings.expand_blacklist = expanded
        self.settings.save()

    def _on_saved_section_toggled(self, expanded: bool) -> None:
        self.settings.expand_saved = expanded
        self.settings.save()

    def _update_blacklist_badge(self) -> None:
        cnt = len(self.settings.quick_blacklist)
        self.bl_section.set_badge(f"{cnt} active" if cnt else "")

    def _update_saved_badge(self) -> None:
        cnt = self.saved_list.count()
        self.saved_section.set_badge(f"{cnt} saved" if cnt else "")

    def _on_quick_blacklist_toggled(self) -> None:
        self.settings.quick_blacklist = [tag for tag, cb in self.quick_bl_checkboxes.items() if cb.isChecked()]
        self.blacklist = Blacklist(full_blacklist(self.settings))
        self._update_blacklist_badge()
        self.settings.save()

    def _select_common_blacklist(self) -> None:
        common = {"gore", "scat", "watersports", "young", "type:swf"}
        for tag, cb in self.quick_bl_checkboxes.items():
            cb.setChecked(tag in common)
        self.settings.quick_blacklist = [tag for tag, cb in self.quick_bl_checkboxes.items() if cb.isChecked()]
        self.blacklist = Blacklist(full_blacklist(self.settings))
        self._update_blacklist_badge()
        self.settings.save()

    def _clear_quick_blacklist(self) -> None:
        for cb in self.quick_bl_checkboxes.values():
            cb.setChecked(False)
        self.settings.quick_blacklist = []
        self.blacklist = Blacklist(full_blacklist(self.settings))
        self._update_blacklist_badge()
        self.settings.save()

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
        self.pool_list.currentItemChanged.connect(self._on_pool_list_selection_changed)
        open_btn = QPushButton("Show pool")
        open_btn.clicked.connect(self._open_selected_pool)
        open_tab_btn = QPushButton("+ New Tab")
        open_tab_btn.setToolTip("Open pool in a new tab")
        open_tab_btn.clicked.connect(lambda: self._open_selected_pool(new_tab=True))
        self.btn_pool_tab_bank = QPushButton("+ Bank")
        self.btn_pool_tab_bank.setToolTip("Save/remove selected pool in Spank Bank")
        self.btn_pool_tab_bank.setStyleSheet("font-weight: bold; color: #a78bfa;")
        self.btn_pool_tab_bank.clicked.connect(self._toggle_selected_pool_in_bank)
        dl_btn = QPushButton("Download pool")
        dl_btn.setStyleSheet("font-weight: bold;")
        dl_btn.clicked.connect(self._download_selected_pool)
        row = QHBoxLayout()
        row.addWidget(self.pool_edit, 1)
        row.addWidget(go)
        lay.addLayout(row)
        lay.addWidget(self.pool_list, 1)
        r2 = QHBoxLayout()
        r2.addWidget(open_btn)
        r2.addWidget(open_tab_btn)
        r2.addWidget(self.btn_pool_tab_bank)
        r2.addWidget(dl_btn)
        lay.addLayout(r2)
        tip = QLabel("<small>Pools are saved to <b>pools/&lt;name&gt; (id)/</b> with pages "
                     "numbered in reading order (0001_…, 0002_…).</small>")
        tip.setWordWrap(True)
        lay.addWidget(tip)
        return w

    def _on_pool_list_selection_changed(self, current: QListWidgetItem | None, _prev: QListWidgetItem | None = None) -> None:
        if not hasattr(self, "btn_pool_tab_bank"):
            return
        pool = current.data(Qt.ItemDataRole.UserRole) if current else None
        if pool and self.spank_bank.has_pool(pool.id):
            self.btn_pool_tab_bank.setText("In Bank")
            self.btn_pool_tab_bank.setStyleSheet("font-weight: bold; color: #fbbf24;")
        else:
            self.btn_pool_tab_bank.setText("+ Bank")
            self.btn_pool_tab_bank.setStyleSheet("font-weight: bold; color: #a78bfa;")

    def _toggle_selected_pool_in_bank(self) -> None:
        if pool := self._selected_pool():
            self._toggle_pool_in_bank(pool)
            if hasattr(self, "pool_list"):
                self._on_pool_list_selection_changed(self.pool_list.currentItem())

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

    def _build_spank_bank_tab(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(4, 4, 4, 4)

        self.bank_subtabs = QTabWidget()
        self.bank_subtabs.addTab(self._build_spank_bank_pools_widget(), "Pools (0)")
        self.bank_subtabs.addTab(self._build_spank_bank_posts_widget(), "Posts (0)")
        self.bank_subtabs.addTab(self._build_spank_bank_artists_widget(), "Artists (0)")

        tip = QLabel("<small>The <b>Spank Bank</b> lets you save online pools, posts and artists for later viewing without "
                     "downloading them locally. Play full-screen slideshows with timers in reading order or shuffle.</small>")
        tip.setWordWrap(True)

        lay.addWidget(self.bank_subtabs, 1)
        lay.addWidget(tip)

        QTimer.singleShot(50, self._refresh_spank_bank_ui)
        return w

    def _build_spank_bank_pools_widget(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(2, 2, 2, 2)

        self.bank_pools_summary_label = QLabel("0 saved pools")
        self.bank_pools_summary_label.setStyleSheet("font-weight: bold; font-size: 13px; color: #fbbf24;")

        self.bank_pools_filter_edit = QLineEdit()
        self.bank_pools_filter_edit.setPlaceholderText("Filter saved pools by name or ID…")
        self.bank_pools_filter_edit.setClearButtonEnabled(True)
        self.bank_pools_filter_edit.textChanged.connect(self._filter_bank_pools_list)

        self.bank_pools_list = QListWidget()
        self.bank_pools_list.itemDoubleClicked.connect(self._open_bank_saved_pool_item)

        btn_row1 = QHBoxLayout()
        view_btn = QPushButton("View Pool")
        view_btn.setStyleSheet("font-weight: bold;")
        view_btn.clicked.connect(lambda: self._open_bank_saved_pool(new_tab=False))
        new_tab_btn = QPushButton("+ New Tab")
        new_tab_btn.clicked.connect(lambda: self._open_bank_saved_pool(new_tab=True))
        btn_row1.addWidget(view_btn)
        btn_row1.addWidget(new_tab_btn)

        btn_row2 = QHBoxLayout()
        slideshow_order_btn = QPushButton("Play In Order")
        slideshow_order_btn.setStyleSheet("font-weight: bold; background-color: #1e3a8a; color: white;")
        slideshow_order_btn.setToolTip("Load pool posts and start full-screen slideshow in reading order")
        slideshow_order_btn.clicked.connect(lambda: self._play_bank_saved_pool_slideshow(shuffle=False))
        slideshow_shuffle_btn = QPushButton("Play Shuffle")
        slideshow_shuffle_btn.setStyleSheet("font-weight: bold; background-color: #7c2d12; color: white;")
        slideshow_shuffle_btn.setToolTip("Load pool posts and start full-screen slideshow shuffled")
        slideshow_shuffle_btn.clicked.connect(lambda: self._play_bank_saved_pool_slideshow(shuffle=True))
        btn_row2.addWidget(slideshow_order_btn)
        btn_row2.addWidget(slideshow_shuffle_btn)

        btn_row3 = QHBoxLayout()
        del_btn = QPushButton("Remove selected")
        del_btn.clicked.connect(self._remove_selected_bank_pools)
        clear_btn = QPushButton("Clear all pools")
        clear_btn.clicked.connect(self._clear_bank_pools)
        btn_row3.addWidget(del_btn)
        btn_row3.addWidget(clear_btn)

        lay.addWidget(self.bank_pools_summary_label)
        lay.addWidget(self.bank_pools_filter_edit)
        lay.addWidget(self.bank_pools_list, 1)
        lay.addLayout(btn_row1)
        lay.addLayout(btn_row2)
        lay.addLayout(btn_row3)
        return w

    def _build_spank_bank_posts_widget(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(2, 2, 2, 2)

        self.bank_summary_label = QLabel("0 saved posts")
        self.bank_summary_label.setStyleSheet("font-weight: bold; font-size: 13px; color: #fbbf24;")

        self.bank_filter_edit = QLineEdit()
        self.bank_filter_edit.setPlaceholderText("Filter saved posts by tag or ID…")
        self.bank_filter_edit.setClearButtonEnabled(True)
        self.bank_filter_edit.textChanged.connect(self._filter_bank_list)

        self.bank_list = QListWidget()
        self.bank_list.itemDoubleClicked.connect(self._open_bank_item)

        btn_row1 = QHBoxLayout()
        view_btn = QPushButton("View Bank")
        view_btn.setStyleSheet("font-weight: bold;")
        view_btn.clicked.connect(lambda: self._open_spank_bank(new_tab=False))
        new_tab_btn = QPushButton("+ New Tab")
        new_tab_btn.clicked.connect(lambda: self._open_spank_bank(new_tab=True))
        btn_row1.addWidget(view_btn)
        btn_row1.addWidget(new_tab_btn)

        btn_row2 = QHBoxLayout()
        slideshow_order_btn = QPushButton("Play In Order")
        slideshow_order_btn.setStyleSheet("font-weight: bold; background-color: #1e3a8a; color: white;")
        slideshow_order_btn.clicked.connect(lambda: self._play_bank_slideshow(shuffle=False))
        slideshow_shuffle_btn = QPushButton("Play Shuffle")
        slideshow_shuffle_btn.setStyleSheet("font-weight: bold; background-color: #7c2d12; color: white;")
        slideshow_shuffle_btn.clicked.connect(lambda: self._play_bank_slideshow(shuffle=True))
        btn_row2.addWidget(slideshow_order_btn)
        btn_row2.addWidget(slideshow_shuffle_btn)

        btn_row3 = QHBoxLayout()
        del_btn = QPushButton("Remove selected")
        del_btn.clicked.connect(self._remove_selected_bank_items)
        clear_btn = QPushButton("Clear all")
        clear_btn.clicked.connect(self._clear_spank_bank)
        btn_row3.addWidget(del_btn)
        btn_row3.addWidget(clear_btn)

        lay.addWidget(self.bank_summary_label)
        lay.addWidget(self.bank_filter_edit)
        lay.addWidget(self.bank_list, 1)
        lay.addLayout(btn_row1)
        lay.addLayout(btn_row2)
        lay.addLayout(btn_row3)
        return w

    def _build_spank_bank_artists_widget(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(2, 2, 2, 2)

        self.bank_artists_summary_label = QLabel("0 saved artists")
        self.bank_artists_summary_label.setStyleSheet("font-weight: bold; font-size: 13px; color: #fbbf24;")

        add_row = QHBoxLayout()
        self.bank_artist_add_edit = QLineEdit()
        self.bank_artist_add_edit.setPlaceholderText("Add artist by name…")
        self.bank_artist_add_edit.returnPressed.connect(self._add_bank_artist_from_edit)
        add_btn = QPushButton("+ Add")
        add_btn.clicked.connect(self._add_bank_artist_from_edit)
        add_row.addWidget(self.bank_artist_add_edit, 1)
        add_row.addWidget(add_btn)

        self.bank_artists_filter_edit = QLineEdit()
        self.bank_artists_filter_edit.setPlaceholderText("Filter saved artists…")
        self.bank_artists_filter_edit.setClearButtonEnabled(True)
        self.bank_artists_filter_edit.textChanged.connect(self._filter_bank_artists_list)

        self.bank_artists_list = QListWidget()
        self.bank_artists_list.setSelectionMode(QListWidget.SelectionMode.ExtendedSelection)
        self.bank_artists_list.itemDoubleClicked.connect(
            lambda it: self._open_artist(it.data(Qt.ItemDataRole.UserRole), new_tab=False))

        btn_row1 = QHBoxLayout()
        view_btn = QPushButton("View Artist")
        view_btn.setStyleSheet("font-weight: bold;")
        view_btn.clicked.connect(lambda: self._open_bank_saved_artist(new_tab=False))
        new_tab_btn = QPushButton("+ New Tab")
        new_tab_btn.clicked.connect(lambda: self._open_bank_saved_artist(new_tab=True))
        web_btn = QPushButton("e621 Page")
        web_btn.setToolTip("Open the artist's page on e621")
        web_btn.clicked.connect(self._open_bank_saved_artist_web)
        btn_row1.addWidget(view_btn)
        btn_row1.addWidget(new_tab_btn)
        btn_row1.addWidget(web_btn)

        feed_btn = QPushButton("Latest From All Saved Artists")
        feed_btn.setStyleSheet("font-weight: bold; color: #fbbf24;")
        feed_btn.setToolTip("One combined feed of the newest posts by every saved artist")
        feed_btn.clicked.connect(lambda: self._open_bank_artists_feed(new_tab=False))

        btn_row2 = QHBoxLayout()
        slideshow_order_btn = QPushButton("Play Newest")
        slideshow_order_btn.setStyleSheet("font-weight: bold; background-color: #1e3a8a; color: white;")
        slideshow_order_btn.setToolTip("Slideshow of the selected artist's newest posts")
        slideshow_order_btn.clicked.connect(lambda: self._play_bank_artist_slideshow(shuffle=False))
        slideshow_shuffle_btn = QPushButton("Play Shuffle")
        slideshow_shuffle_btn.setStyleSheet("font-weight: bold; background-color: #7c2d12; color: white;")
        slideshow_shuffle_btn.setToolTip("Shuffled slideshow of the selected artist's posts")
        slideshow_shuffle_btn.clicked.connect(lambda: self._play_bank_artist_slideshow(shuffle=True))
        btn_row2.addWidget(slideshow_order_btn)
        btn_row2.addWidget(slideshow_shuffle_btn)

        btn_row3 = QHBoxLayout()
        del_btn = QPushButton("Remove selected")
        del_btn.clicked.connect(self._remove_selected_bank_artists)
        clear_btn = QPushButton("Clear all artists")
        clear_btn.clicked.connect(self._clear_bank_artists)
        btn_row3.addWidget(del_btn)
        btn_row3.addWidget(clear_btn)

        lay.addWidget(self.bank_artists_summary_label)
        lay.addLayout(add_row)
        lay.addWidget(self.bank_artists_filter_edit)
        lay.addWidget(self.bank_artists_list, 1)
        lay.addLayout(btn_row1)
        lay.addWidget(feed_btn)
        lay.addLayout(btn_row2)
        lay.addLayout(btn_row3)
        return w

    def _refresh_spank_bank_ui(self) -> None:
        if hasattr(self, "bank_list") and hasattr(self, "bank_summary_label"):
            posts = self.spank_bank.get_all()
            cnt = len(posts)
            self.bank_summary_label.setText(f"{cnt} saved post{'s' if cnt != 1 else ''}")
            filter_txt = self.bank_filter_edit.text() if hasattr(self, "bank_filter_edit") else ""
            self._filter_bank_list(filter_txt)
        if hasattr(self, "bank_pools_list") and hasattr(self, "bank_pools_summary_label"):
            pools = self.spank_bank.get_all_pools()
            p_cnt = len(pools)
            self.bank_pools_summary_label.setText(f"{p_cnt} saved pool{'s' if p_cnt != 1 else ''}")
            p_filter_txt = self.bank_pools_filter_edit.text() if hasattr(self, "bank_pools_filter_edit") else ""
            self._filter_bank_pools_list(p_filter_txt)
        if hasattr(self, "bank_artists_list") and hasattr(self, "bank_artists_summary_label"):
            a_cnt = self.spank_bank.artist_count
            self.bank_artists_summary_label.setText(f"{a_cnt} saved artist{'s' if a_cnt != 1 else ''}")
            self._filter_bank_artists_list(self.bank_artists_filter_edit.text())
        if hasattr(self, "bank_subtabs"):
            self.bank_subtabs.setTabText(0, f"Pools ({self.spank_bank.pool_count})")
            self.bank_subtabs.setTabText(1, f"Posts ({len(self.spank_bank)})")
            self.bank_subtabs.setTabText(2, f"Artists ({self.spank_bank.artist_count})")

    def _filter_bank_artists_list(self, text: str = "") -> None:
        if not hasattr(self, "bank_artists_list"):
            return
        text = text.lower().strip().replace(" ", "_")
        self.bank_artists_list.clear()
        for name in self.spank_bank.get_all_artists():
            if not text or text in name:
                it = QListWidgetItem(name)
                it.setData(Qt.ItemDataRole.UserRole, name)
                it.setToolTip(f"Double-click to view all posts by {name}")
                self.bank_artists_list.addItem(it)

    def _filter_bank_list(self, text: str = "") -> None:
        if not hasattr(self, "bank_list"):
            return
        text = text.lower().strip()
        self.bank_list.clear()
        for p in self.spank_bank.get_all():
            artists = ", ".join(p.artists) or "unknown"
            display = f"#{p.id} · {artists} · Score {p.score} · {p.rating.upper()}"
            all_tags = " ".join(t for tags in p.tags.values() for t in tags).lower()
            if not text or text in str(p.id) or text in artists.lower() or text in all_tags:
                it = QListWidgetItem(display)
                it.setData(Qt.ItemDataRole.UserRole, p)
                it.setToolTip(f"Double-click to open #{p.id}")
                self.bank_list.addItem(it)

    def _filter_bank_pools_list(self, text: str = "") -> None:
        if not hasattr(self, "bank_pools_list"):
            return
        text = text.lower().strip()
        self.bank_pools_list.clear()
        for pool in self.spank_bank.get_all_pools():
            display = f"#{pool.id} · {pool.display_name} ({pool.post_count} posts)"
            if not text or text in str(pool.id) or text in pool.name.lower() or text in pool.category.lower():
                it = QListWidgetItem(display)
                it.setData(Qt.ItemDataRole.UserRole, pool)
                it.setToolTip(f"Double-click to open pool #{pool.id}: {pool.display_name}")
                self.bank_pools_list.addItem(it)

    def _selected_bank_saved_pool(self) -> Pool | None:
        if not hasattr(self, "bank_pools_list"):
            return None
        it = self.bank_pools_list.currentItem()
        return it.data(Qt.ItemDataRole.UserRole) if it else None

    def _open_bank_saved_pool_item(self, item: QListWidgetItem) -> None:
        pool = item.data(Qt.ItemDataRole.UserRole)
        if pool:
            self._open_pool(pool, new_tab=False)

    def _open_bank_saved_pool(self, new_tab: bool = False) -> None:
        pool = self._selected_bank_saved_pool()
        if not pool:
            QMessageBox.information(self, "No pool selected", "Select a pool in the saved pools list first.")
            return
        self._open_pool(pool, new_tab=new_tab)

    def _play_bank_saved_pool_slideshow(self, shuffle: bool = False) -> None:
        pool = self._selected_bank_saved_pool()
        if not pool:
            QMessageBox.information(self, "No pool selected", "Select a pool in the saved pools list first.")
            return
        s = self._source
        if s.kind == "pool" and s.pool and s.pool.id == pool.id and self.grid.count() > 0:
            posts = self.grid.all_posts()
            if shuffle:
                import random
                shuffled = list(posts)
                random.shuffle(shuffled)
                self._preview(shuffled[0], start_slideshow=True, shuffle=True)
            else:
                self._preview(posts[0], start_slideshow=True, shuffle=False)
            return

        self.statusBar().showMessage(f"Loading pool '{pool.display_name}' for slideshow…")
        def on_posts(posts: list[Post]):
            filtered_posts, _ = self.blacklist.filter(posts)
            if not filtered_posts:
                QMessageBox.information(self, "Empty pool", "No posts available to display in this pool.")
                return
            if shuffle:
                import random
                shuffled = list(filtered_posts)
                random.shuffle(shuffled)
                self._preview(shuffled[0], start_slideshow=True, shuffle=True, posts=shuffled)
            else:
                self._preview(filtered_posts[0], start_slideshow=True, shuffle=False, posts=filtered_posts)

        self.api.submit(self.client.get_pool_posts, pool,
                        on_result=on_posts,
                        on_error=partial(self._on_api_error, self._gen))

    def _remove_selected_bank_pools(self) -> None:
        items = self.bank_pools_list.selectedItems()
        if not items:
            return
        for it in items:
            p = it.data(Qt.ItemDataRole.UserRole)
            if p:
                self.spank_bank.remove_pool(p.id)
                self._update_all_pool_bank_indicators(p.id, False)
        self._refresh_spank_bank_ui()
        self.statusBar().showMessage(f"Removed {len(items)} pool(s) from Spank Bank", 3000)

    def _clear_bank_pools(self) -> None:
        if self.spank_bank.pool_count == 0:
            return
        r = QMessageBox.question(self, "Clear Saved Pools",
                                 f"Are you sure you want to clear all {self.spank_bank.pool_count} saved pools?")
        if r == QMessageBox.StandardButton.Yes:
            self.spank_bank.clear_pools()
            self._refresh_spank_bank_ui()
            self.statusBar().showMessage("Cleared all saved pools from Spank Bank", 3000)

    def _toggle_pool_in_bank(self, pool: Pool) -> bool:
        added = self.spank_bank.toggle_pool(pool)
        self._refresh_spank_bank_ui()
        self._update_all_pool_bank_indicators(pool.id, added)
        msg = f"Saved pool '{pool.display_name}' to Spank Bank" if added else f"Removed pool '{pool.display_name}' from Spank Bank"
        self.statusBar().showMessage(msg, 3000)
        self._log(msg)
        return added

    def _toggle_current_pool_in_bank(self) -> None:
        """Tab nav bank button: toggles the current pool or artist."""
        s = self._source
        if s.kind == "pool" and s.pool:
            self._toggle_pool_in_bank(s.pool)
        elif s.kind == "artist":
            self._toggle_artist_in_bank(s.label)

    def _toggle_pool_by_id_in_bank(self, pool_id: int) -> None:
        if self.spank_bank.has_pool(pool_id):
            self.spank_bank.remove_pool(pool_id)
            self._refresh_spank_bank_ui()
            self._update_all_pool_bank_indicators(pool_id, False)
            msg = f"Removed pool #{pool_id} from Spank Bank"
            self.statusBar().showMessage(msg, 3000)
            self._log(msg)
        else:
            s = self._source
            if s.kind == "pool" and s.pool and s.pool.id == pool_id:
                self._toggle_pool_in_bank(s.pool)
            else:
                self.statusBar().showMessage(f"Fetching pool #{pool_id} metadata…")
                def on_pool(pool: Pool):
                    self._toggle_pool_in_bank(pool)
                self.api.submit(self.client.get_pool, pool_id, on_result=on_pool,
                                on_error=partial(self._on_api_error, self._gen))

    def _update_all_pool_bank_indicators(self, pool_id: int, in_bank: bool) -> None:
        tab = self.current_tab
        if tab.source.kind == "pool" and tab.source.pool and tab.source.pool.id == pool_id:
            tab.update_nav_buttons(in_bank=in_bank)
        if hasattr(self, "inspector") and self.inspector._current_post:
            if pool_id in self.inspector._current_post.pools:
                self.inspector.set_pool_in_bank(in_bank)
        if self._preview_dlg and self._preview_dlg.isVisible():
            if pool_id in self._preview_dlg.post.pools:
                self._preview_dlg.set_pool_in_bank(in_bank)
        if hasattr(self, "pool_list") and hasattr(self, "btn_pool_tab_bank"):
            self._on_pool_list_selection_changed(self.pool_list.currentItem())

    def _open_bank_item(self, item: QListWidgetItem) -> None:
        post = item.data(Qt.ItemDataRole.UserRole)
        if post:
            self._preview(post)

    def _open_spank_bank(self, new_tab: bool = False) -> None:
        posts = self.spank_bank.get_all()
        if not posts:
            QMessageBox.information(self, "Spank Bank empty",
                                    "No posts have been added to the Spank Bank yet.\n"
                                    "Click '+ Bank' on any post or press 'B' in preview to save it.")
            return
        src = Source("spank_bank", "Spank Bank", "spank_bank")
        self._begin(src, new_tab=new_tab)
        self._show_posts(posts)
        self._exhausted = True
        self._update_result_label()
        self.statusBar().showMessage(f"Loaded {len(posts)} posts from Spank Bank", 3000)

    def _play_bank_slideshow(self, shuffle: bool = False) -> None:
        posts = self.spank_bank.get_all()
        if not posts:
            QMessageBox.information(self, "Spank Bank empty", "No posts in Spank Bank to play.")
            return
        if shuffle:
            import random
            shuffled = list(posts)
            random.shuffle(shuffled)
            self._preview(shuffled[0], start_slideshow=True, shuffle=True, posts=shuffled)
        else:
            self._preview(posts[0], start_slideshow=True, shuffle=False, posts=posts)

    def _start_slideshow(self, shuffle: bool = False) -> None:
        posts = self.grid.all_posts()
        if not posts:
            QMessageBox.information(self, "No posts", "No posts in the current view to play in a slideshow.")
            return
        self._preview(posts[0], start_slideshow=True, shuffle=shuffle)

    def _remove_selected_bank_items(self) -> None:
        items = self.bank_list.selectedItems()
        if not items:
            return
        for it in items:
            p = it.data(Qt.ItemDataRole.UserRole)
            if p:
                self.spank_bank.remove(p.id)
        self._refresh_spank_bank_ui()
        self.statusBar().showMessage(f"Removed {len(items)} post(s) from Spank Bank", 3000)

    def _clear_spank_bank(self) -> None:
        if len(self.spank_bank) == 0:
            return
        r = QMessageBox.question(self, "Clear Spank Bank",
                                 f"Are you sure you want to clear all {len(self.spank_bank)} saved posts?")
        if r == QMessageBox.StandardButton.Yes:
            self.spank_bank.clear()
            self._refresh_spank_bank_ui()
            self.statusBar().showMessage("Cleared Spank Bank", 3000)

    def _toggle_spank_bank(self, post: Post) -> bool:
        added = self.spank_bank.toggle(post)
        if self._preview_dlg and self._preview_dlg.isVisible():
            if self._preview_dlg.post.id == post.id:
                self._preview_dlg.set_in_bank(added)
        if self.inspector._current_post and self.inspector._current_post.id == post.id:
            self.inspector.set_in_bank(added)
        self._refresh_spank_bank_ui()
        msg = f"Saved post #{post.id} to Spank Bank" if added else f"Removed post #{post.id} from Spank Bank"
        self.statusBar().showMessage(msg, 3000)
        self._log(msg)
        return added

    # ================================================================ artists
    @staticmethod
    def _norm_artist(name: str) -> str:
        return (name or "").strip().lower().replace(" ", "_")

    def _open_artist(self, name: str, new_tab: bool = False) -> None:
        """Show every post by an artist (like the artist link on e621), paginated."""
        name = self._norm_artist(name)
        if not name:
            return
        self._start_search(name, label=name, kind="artist", new_tab=new_tab)
        self.statusBar().showMessage(f"Loading posts by {name}…")

    def _source_in_bank(self, s: Source) -> bool:
        if s.kind == "pool" and s.pool:
            return self.spank_bank.has_pool(s.pool.id)
        if s.kind == "artist":
            return self.spank_bank.has_artist(s.label)
        return False

    def _toggle_artist_in_bank(self, name: str) -> bool:
        name = self._norm_artist(name)
        if not name:
            return False
        added = self.spank_bank.toggle_artist(name)
        self._refresh_spank_bank_ui()
        self._update_all_artist_bank_indicators()
        msg = f"Saved artist '{name}' to Spank Bank" if added else f"Removed artist '{name}' from Spank Bank"
        self.statusBar().showMessage(msg, 3000)
        self._log(msg)
        return added

    def _update_all_artist_bank_indicators(self) -> None:
        for i in range(self.results_tabs.count()):
            tab = self.results_tabs.widget(i)
            if isinstance(tab, ResultTab) and tab.source.kind == "artist":
                tab.update_nav_buttons(in_bank=self._source_in_bank(tab.source))
        if p := self.inspector._current_post:
            self.inspector.set_artists_banked({a for a in p.artists if self.spank_bank.has_artist(a)})
        if self._preview_dlg and self._preview_dlg.isVisible():
            self._preview_dlg.refresh_artist_bank_state()

    def _add_bank_artist_from_edit(self) -> None:
        name = self._norm_artist(self.bank_artist_add_edit.text())
        if not name:
            return
        if not self.spank_bank.has_artist(name):
            self._toggle_artist_in_bank(name)
        self.bank_artist_add_edit.clear()

    def _selected_bank_artist(self) -> str | None:
        it = self.bank_artists_list.currentItem()
        return it.data(Qt.ItemDataRole.UserRole) if it else None

    def _open_bank_saved_artist(self, new_tab: bool = False) -> None:
        name = self._selected_bank_artist()
        if not name:
            QMessageBox.information(self, "No artist selected", "Select an artist in the saved artists list first.")
            return
        self._open_artist(name, new_tab=new_tab)

    def _open_bank_saved_artist_web(self) -> None:
        if name := self._selected_bank_artist():
            QDesktopServices.openUrl(QUrl(artist_page_url(name)))

    # e621 allows 40 tags per search; leave headroom for rating/order/blacklist tags.
    MAX_FEED_ARTISTS = 25

    def _open_bank_artists_feed(self, new_tab: bool = False) -> None:
        artists = self.spank_bank.get_all_artists()
        if not artists:
            QMessageBox.information(self, "No saved artists",
                                    "No artists in the Spank Bank yet.\n"
                                    "Right-click an artist tag or use '+ Bank Artist' on any post.")
            return
        used = artists[:self.MAX_FEED_ARTISTS]
        raw = used[0] if len(used) == 1 else " ".join(f"~{a}" for a in used)
        self._start_search(raw, label="saved_artists", kind="artist_feed", new_tab=new_tab)
        if len(artists) > len(used):
            self.statusBar().showMessage(
                f"e621 limits tags per search: feed shows the {len(used)} most recently saved artists", 6000)

    def _play_bank_artist_slideshow(self, shuffle: bool = False) -> None:
        name = self._selected_bank_artist()
        if not name:
            QMessageBox.information(self, "No artist selected", "Select an artist in the saved artists list first.")
            return
        self.statusBar().showMessage(f"Loading posts by {name} for slideshow…")

        def on_posts(posts: list[Post]):
            kept, _ = self.blacklist.filter(posts)
            if not kept:
                QMessageBox.information(self, "No posts", f"No posts available to display for {name}.")
                return
            order = list(kept)
            if shuffle:
                import random
                random.shuffle(order)
            self._preview(order[0], start_slideshow=True, shuffle=shuffle, posts=order)

        self.api.submit(self.client.search_posts, self._compose_query(name), 1,
                        on_result=on_posts, on_error=partial(self._on_api_error, self._gen))

    def _remove_selected_bank_artists(self) -> None:
        items = self.bank_artists_list.selectedItems()
        if not items:
            return
        for it in items:
            self.spank_bank.remove_artist(it.data(Qt.ItemDataRole.UserRole))
        self._refresh_spank_bank_ui()
        self._update_all_artist_bank_indicators()
        self.statusBar().showMessage(f"Removed {len(items)} artist(s) from Spank Bank", 3000)

    def _clear_bank_artists(self) -> None:
        if self.spank_bank.artist_count == 0:
            return
        r = QMessageBox.question(self, "Clear Saved Artists",
                                 f"Are you sure you want to clear all {self.spank_bank.artist_count} saved artists?")
        if r == QMessageBox.StandardButton.Yes:
            self.spank_bank.clear_artists()
            self._refresh_spank_bank_ui()
            self._update_all_artist_bank_indicators()
            self.statusBar().showMessage("Cleared all saved artists from Spank Bank", 3000)

    # ============================================================== e621 interactions
    def _prompt_login_for_interaction(self, action_name: str = "Interacting with posts") -> bool:
        if self.client.authenticated:
            return True
        r = QMessageBox.question(
            self,
            "e621 Login Required",
            f"{action_name} on e621 requires logging in with your e621 Username and API key so the site can register your action and credit the artist.\n\n"
            "Would you like to open Settings now to enter your credentials?",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
        )
        if r == QMessageBox.StandardButton.Yes:
            self._open_settings()
        return False

    def _toggle_favorite(self, post: Post) -> None:
        if not self._prompt_login_for_interaction("Favoriting / liking posts"):
            return
        is_fav = post.is_favorited
        action_verb = "Removing favorite from" if is_fav else "Favoriting"
        self.statusBar().showMessage(f"{action_verb} post #{post.id} on e621…")

        def call():
            if is_fav:
                self.client.remove_favorite(post.id)
                return False
            else:
                self.client.add_favorite(post.id)
                return True

        def on_done(new_state: bool):
            post.is_favorited = new_state
            if new_state:
                post.fav_count += 1
                msg = f"Favorited post #{post.id} on e621 (artist credited)"
            else:
                post.fav_count = max(0, post.fav_count - 1)
                msg = f"Removed post #{post.id} from your e621 favorites"
            self._update_post_interactions_ui(post)
            self.statusBar().showMessage(msg, 4000)
            self._log(msg)

        def on_err(e):
            err_msg = str(e)
            self.statusBar().showMessage(f"Failed to update favorite: {err_msg}", 5000)
            QMessageBox.warning(self, "Favorite Failed", f"Could not update favorite on e621:\n{err_msg}")

        self.api.submit(call, on_result=on_done, on_error=on_err)

    def _toggle_upvote(self, post: Post) -> None:
        if not self._prompt_login_for_interaction("Voting on posts"):
            return
        is_voted = (post.own_vote == 1)
        action_verb = "Removing upvote from" if is_voted else "Upvoting"
        self.statusBar().showMessage(f"{action_verb} post #{post.id} on e621…")

        def call():
            if is_voted:
                self.client.remove_vote(post.id)
                return 0
            else:
                self.client.vote_post(post.id, 1)
                return 1

        def on_done(new_vote: int):
            if new_vote == 1:
                post.score += 1
                post.own_vote = 1
                msg = f"Upvoted post #{post.id} on e621 (+1)"
            else:
                post.score -= 1
                post.own_vote = 0
                msg = f"Removed upvote from post #{post.id} on e621"
            self._update_post_interactions_ui(post)
            self.statusBar().showMessage(msg, 4000)
            self._log(msg)

        def on_err(e):
            err_msg = str(e)
            self.statusBar().showMessage(f"Failed to vote: {err_msg}", 5000)
            QMessageBox.warning(self, "Vote Failed", f"Could not vote on e621:\n{err_msg}")

        self.api.submit(call, on_result=on_done, on_error=on_err)

    def _update_post_interactions_ui(self, post: Post) -> None:
        if hasattr(self, "inspector") and self.inspector._current_post:
            if self.inspector._current_post.id == post.id:
                self.inspector.update_interactions(post)
        if self._preview_dlg and self._preview_dlg.isVisible():
            if self._preview_dlg.post.id == post.id:
                self._preview_dlg.update_interactions()

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
        media_idx = self.media_filter.findData(s.media_filter)
        self.media_filter.setCurrentIndex(max(0, media_idx))
        self.cb_in_pool.setChecked(s.pool_only)
        for tag, cb in self.quick_bl_checkboxes.items():
            cb.setChecked(tag in s.quick_blacklist)
        self.bl_section.set_expanded(s.expand_blacklist)
        self.saved_section.set_expanded(s.expand_saved)
        self._update_blacklist_badge()
        self.min_score.setValue(s.min_score)
        self.order.setCurrentIndex(max(0, self.order.findData(s.order)))
        self.fav_user.setText(s.favorites_user)
        self.saved_list.clear()
        self.saved_list.addItems(s.saved_searches)
        self._update_saved_badge()

    def _save_ui_to_settings(self) -> None:
        s = self.settings
        s.download_dir = self.dir_edit.text().strip() or s.download_dir
        s.organize_mode = self.organize.currentData()
        s.write_tags_file = self.tags_file_cb.isChecked()
        s.rating_s, s.rating_q, s.rating_e = (self.cb_s.isChecked(), self.cb_q.isChecked(),
                                              self.cb_e.isChecked())
        s.media_filter = self.media_filter.currentData() or ""
        s.pool_only = self.cb_in_pool.isChecked()
        s.quick_blacklist = [tag for tag, cb in self.quick_bl_checkboxes.items() if cb.isChecked()]
        s.expand_blacklist = self.bl_section.is_expanded()
        s.expand_saved = self.saved_section.is_expanded()
        s.min_score = self.min_score.value()
        s.order = self.order.currentData()
        s.favorites_user = self.fav_user.text().strip()
        s.saved_searches = [self.saved_list.item(i).text() for i in range(self.saved_list.count())]
        s.window_geometry = bytes(self.saveGeometry().toBase64()).decode()
        if hasattr(self, "split"):
            s.splitter_state = bytes(self.split.saveState().toBase64()).decode()

    def _apply_settings(self) -> None:
        s = self.settings
        self.client = E621Client(s.username, s.api_key)
        self.blacklist = Blacklist(full_blacklist(s))
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
                self.statusBar().showMessage(f"[OK] {msg}", 6000)
                QMessageBox.information(self, "Connection Successful", msg)
            else:
                self.statusBar().showMessage(f"[Failed] {msg}", 8000)
                QMessageBox.warning(self, "Connection Failed", msg)
        self.api.submit(check, on_result=on_checked)

    def _initial_connect(self) -> None:
        def check():
            return self.client.check_connection()
        def on_checked(res):
            ok, msg = res
            if ok:
                self.statusBar().showMessage(f"[OK] {msg}", 5000)
                self.auth_label.setText(self.settings.username or "Anonymous")
            else:
                self.statusBar().showMessage(f"[Failed] {msg}", 8000)
                self.auth_label.setText("Connection error")
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

        # Media filter
        media_val = self.media_filter.currentData()
        if media_val and media_val not in base:
            parts.append(media_val)

        # In pool only
        if self.cb_in_pool.isChecked() and "pool:" not in base:
            parts.append("pool:any")

        # Quick blacklist server-side exclusions
        active_bl = [tag for tag, cb in self.quick_bl_checkboxes.items() if cb.isChecked()]
        for tag in active_bl:
            neg = f"-{tag}"
            if neg not in base and tag not in base:
                parts.append(neg)

        return " ".join(parts).strip()

    def _autocomplete(self, token: str) -> None:
        self.tag_api.submit(self.client.autocomplete, token,
                            on_result=lambda tags: self.tag_edit.set_suggestions(token, tags),
                            on_error=lambda e: None)

    # ============================================================== tab management
    def _create_tab(self, source: Source, title: str = "Search") -> ResultTab:
        tab = ResultTab(source=source)
        tab.back_requested.connect(self._navigate_back)
        tab.forward_requested.connect(self._navigate_forward)
        tab.download_pool_requested.connect(self._download_current_pool)
        tab.bank_pool_requested.connect(self._toggle_current_pool_in_bank)
        tab.slideshow_requested.connect(self._start_slideshow)
        tab.grid.itemDoubleClicked.connect(lambda it: self._preview(tab.grid.post_for(it)))
        tab.grid.customContextMenuRequested.connect(self._grid_menu)
        tab.grid.itemSelectionChanged.connect(self._update_selection_label)
        tab.grid.currentItemChanged.connect(self._on_grid_current_changed)
        tab.grid.near_bottom.connect(self._load_next_page)

        idx = self.results_tabs.addTab(tab, title)
        self.results_tabs.setCurrentIndex(idx)
        return tab

    def _close_tab(self, index: int) -> None:
        if self.results_tabs.count() <= 1:
            tab = self.current_tab
            tab.history_back.clear()
            tab.history_forward.clear()
            self._begin(Source("search", "latest", ""))
            return
        widget = self.results_tabs.widget(index)
        self.results_tabs.removeTab(index)
        if widget:
            widget.deleteLater()

    def _close_current_tab(self) -> None:
        idx = self.results_tabs.currentIndex()
        if idx >= 0:
            self._close_tab(idx)

    def _new_empty_tab(self) -> ResultTab:
        tab = self._create_tab(Source("search", "latest", ""), "New Tab")
        self.tag_edit.setFocus()
        return tab

    def _on_tab_changed(self, idx: int) -> None:
        if idx < 0:
            return
        tab = self.current_tab
        if hasattr(self, "dl_sel_btn"):
            self._update_selection_label()
        curr = tab.grid.currentItem()
        if hasattr(self, "inspector"):
            self._on_grid_current_changed(curr)

    def _update_tab_title(self, tab: ResultTab) -> None:
        idx = self.results_tabs.indexOf(tab)
        if idx < 0:
            return
        s = tab.source
        if s.kind == "pool" and s.pool:
            title = f"Pool: {s.pool.display_name[:20]}"
        elif s.kind == "favorites":
            title = f"Fav: {s.label.removeprefix('favorites_')[:20]}"
        elif s.kind == "spank_bank":
            title = "Spank Bank"
        elif s.kind == "artist":
            title = f"Artist: {s.label[:20]}"
        elif s.kind == "artist_feed":
            title = "Saved Artists"
        elif s.query:
            title = s.query[:20]
        else:
            title = "Latest"
        self.results_tabs.setTabText(idx, title)
        self.results_tabs.setTabToolTip(idx, f"{s.kind.capitalize()}: {s.label or s.query}")

    # ============================================================== navigation history
    def _save_current_snapshot(self) -> TabSnapshot:
        tab = self.current_tab
        posts = tab.grid.all_posts()
        scroll_pos = tab.grid.verticalScrollBar().value()
        return TabSnapshot(
            source=tab.source,
            posts=posts,
            page=tab.page,
            exhausted=tab.exhausted,
            hidden=tab.hidden,
            scroll_pos=scroll_pos,
        )

    def _restore_snapshot(self, tab: ResultTab, snap: TabSnapshot) -> None:
        tab.gen += 1
        self._stop_fetch = True
        tab.source = snap.source
        tab.page = snap.page
        tab.exhausted = snap.exhausted
        tab.hidden = snap.hidden
        tab.loading = False
        tab.grid.clear_posts()
        self.inspector.set_post(None)

        if snap.posts:
            tab.grid.add_posts(snap.posts, {p.id for p in snap.posts if self.history.has(p.id)})
            for p in snap.posts:
                if p.id in self._cached_thumbs:
                    tab.grid.set_thumbnail(p.id, self._cached_thumbs[p.id])
                elif p.preview_url:
                    self.thumbs.submit(fetch_image, p.preview_url,
                                       on_result=partial(self._on_thumb, tab.gen, p.id),
                                       on_error=lambda e: None)
            tab.grid.verticalScrollBar().setValue(snap.scroll_pos)

        tab.update_nav_buttons(in_bank=self._source_in_bank(tab.source))
        self._update_tab_title(tab)
        self._update_result_label()
        self._update_selection_label()

    def _navigate_back(self) -> None:
        tab = self.current_tab
        if not tab.history_back:
            return
        current_snap = self._save_current_snapshot()
        tab.history_forward.append(current_snap)
        prev_snap = tab.history_back.pop()
        self._restore_snapshot(tab, prev_snap)

    def _navigate_forward(self) -> None:
        tab = self.current_tab
        if not tab.history_forward:
            return
        current_snap = self._save_current_snapshot()
        tab.history_back.append(current_snap)
        next_snap = tab.history_forward.pop()
        self._restore_snapshot(tab, next_snap)

    # ============================================================== search & load
    def _start_search(self, raw: str | bool | None = None, label: str | None = None,
                      kind: str = "search", new_tab: bool = False) -> None:
        if raw is None or isinstance(raw, bool):
            raw = self.tag_edit.text().strip()
        else:
            raw = str(raw).strip()
        if not any((self.cb_s.isChecked(), self.cb_q.isChecked(), self.cb_e.isChecked())):
            QMessageBox.warning(self, "No rating", "Select at least one rating.")
            return
        self._begin(Source(kind, label or raw or "latest", self._compose_query(raw)), new_tab=new_tab)
        self._exhausted = False
        self._load_next_page()

    def _begin(self, source: Source, new_tab: bool = False) -> None:
        if new_tab:
            tab = self._create_tab(source, "Loading…")
        else:
            tab = self.current_tab
            if tab.grid.count() > 0 or (tab.source and tab.source.query):
                tab.history_back.append(self._save_current_snapshot())
                tab.history_forward.clear()

        tab.gen += 1
        self._stop_fetch = True
        self.thumbs.clear_pending()
        tab.grid.clear_posts()
        self.inspector.set_post(None)
        tab.source = source
        tab.page = 1
        tab.hidden = 0
        tab.loading = False
        tab.exhausted = True
        tab.update_nav_buttons()
        self._update_tab_title(tab)
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
        tab = self.current_tab
        kept, hidden = self.blacklist.filter(posts)
        for p in kept:
            self.tag_edit.learn_post_tags(p)
        tab.hidden += hidden
        added = tab.grid.add_posts(kept, {p.id for p in kept if self.history.has(p.id)})
        gen = tab.gen
        for p in added:
            if p.id in self._cached_thumbs:
                tab.grid.set_thumbnail(p.id, self._cached_thumbs[p.id])
            elif p.preview_url:
                self.thumbs.submit(fetch_image, p.preview_url,
                                   on_result=partial(self._on_thumb, gen, p.id),
                                   on_error=lambda e: None)
        self._update_result_label()

    def _on_thumb(self, gen: int, post_id: int, img: QImage) -> None:
        self._cached_thumbs[post_id] = img
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
        self.result_label.setText(f"<span style='color: #e06c75;'>Error loading results: {err}</span>")

    def _update_result_label(self) -> None:
        tab = self.current_tab
        s = tab.source
        if s.kind == "pool" and s.pool:
            head = f"Pool: <b>{s.pool.display_name}</b> ({s.pool.post_count} posts)"
        elif s.kind == "favorites":
            head = f"Favorites of <b>{s.label.removeprefix('favorites_')}</b>"
        elif s.kind == "spank_bank":
            head = f"Spank Bank · <b>{len(self.spank_bank)} saved</b>"
        elif s.kind == "artist":
            head = (f"Artist: <b>{html.escape(s.label)}</b> "
                    f"(<a href='{artist_page_url(s.label)}' style='color:#e6a000;'>e621 page</a>)")
        elif s.kind == "artist_feed":
            head = "Latest from <b>saved artists</b>"
        else:
            head = f"Search: <b>{s.query or '(latest posts)'}</b>"
        more = "" if tab.exhausted else " (scroll for more)"
        hidden = f" · {tab.hidden} hidden by blacklist" if tab.hidden else ""
        tab.result_label.setText(f"{head} · {tab.grid.count()} shown{more}{hidden}")
        tab.update_nav_buttons(in_bank=self._source_in_bank(s))

    def _update_selection_label(self) -> None:
        if hasattr(self, "dl_sel_btn"):
            self.dl_sel_btn.setText(f"Download selected ({len(self.grid.selectedItems())})")

    # --------------------------------------------------------- saved searches
    def _save_search(self) -> None:
        q = self.tag_edit.text().strip()
        existing = [self.saved_list.item(i).text() for i in range(self.saved_list.count())]
        if q and q not in existing:
            self.saved_list.addItem(q)
            self._update_saved_badge()
            self._save_ui_to_settings()
            self.settings.save()

    def _remove_saved(self) -> None:
        for it in self.saved_list.selectedItems():
            self.saved_list.takeItem(self.saved_list.row(it))
        self._update_saved_badge()
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

    def _open_selected_pool(self, new_tab: bool = False) -> None:
        if pool := self._selected_pool():
            self._open_pool(pool, new_tab=new_tab)

    def _open_pool(self, pool: Pool, new_tab: bool = False) -> None:
        self._begin(Source("pool", pool.display_name, f"pool:{pool.id}", pool), new_tab=new_tab)
        self.statusBar().showMessage(f"Loading pool {pool.display_name}…")
        self.api.submit(self.client.get_pool_posts, pool,
                        on_result=partial(self._on_pool_posts, self._gen),
                        on_error=partial(self._on_api_error, self._gen))

    def _on_pool_posts(self, gen: int, posts: list[Post]) -> None:
        if gen != self._gen:
            return
        self._show_posts(posts)
        self.statusBar().showMessage("Pool loaded", 3000)

    def _open_pool_by_id(self, pool_id: int, new_tab: bool = False) -> None:
        self.api.submit(self.client.get_pool, pool_id,
                        on_result=lambda pool: self._open_pool(pool, new_tab=new_tab),
                        on_error=partial(self._on_api_error, self._gen))

    def _download_current_pool(self) -> None:
        s = self._source
        if s.kind == "pool" and s.pool:
            self._download_pool_by_id(s.pool.id)
        else:
            QMessageBox.information(self, "Not in a pool", "The current view is not a pool.")

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

    def _download_pool_by_id(self, pool_id: int) -> None:
        if self._source.kind == "pool" and self._source.pool and self._source.pool.id == pool_id \
                and self.grid.count():
            self._enqueue(self.grid.all_posts(), self._source)
            return
        self.statusBar().showMessage(f"Fetching pool #{pool_id} for download…")
        def on_pool(pool: Pool):
            src = Source("pool", pool.display_name, f"pool:{pool.id}", pool)
            self.api.submit(self.client.get_pool_posts, pool,
                            on_result=lambda posts: self._enqueue(self.blacklist.filter(posts)[0], src),
                            on_error=partial(self._on_api_error, self._gen))
        self.api.submit(self.client.get_pool, pool_id, on_result=on_pool,
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
            self._log(f"[OK] #{post_id} -> {detail}")
        elif status == "error":
            self._log(f"[Error] #{post_id}: {detail}")
        elif status == "skipped":
            self._log(f"[Skipped] #{post_id} ({detail})")

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
        in_bank = self.spank_bank.has(post.id)
        pool_in_bank = any(self.spank_bank.has_pool(pid) for pid in post.pools) if post.pools else False
        artists_banked = {a for a in post.artists if self.spank_bank.has_artist(a)}
        self.inspector.set_post(post, img, self.tag_edit.text(), in_bank=in_bank, pool_in_bank=pool_in_bank,
                                artists_banked=artists_banked)

    def _preview(self, post: Post, start_slideshow: bool = False, shuffle: bool = False,
                 posts: list[Post] | None = None) -> PreviewDialog:
        """Open the full-screen viewer. ``posts`` overrides the navigation list (defaults to the grid)."""
        all_posts = list(posts) if posts else self.grid.all_posts()
        if not all_posts:
            all_posts = [post]
        idx = all_posts.index(post) if post in all_posts else 0
        in_bank = self.spank_bank.has(post.id)
        pool_in_bank = any(self.spank_bank.has_pool(pid) for pid in post.pools) if post.pools else False
        dlg = PreviewDialog(post, all_posts=all_posts, index=idx,
                            active_query=self.tag_edit.text(), in_bank=in_bank, pool_in_bank=pool_in_bank,
                            start_slideshow=start_slideshow, shuffle=shuffle,
                            artist_in_bank=self.spank_bank.has_artist, parent=self)
        self._preview_dlg = dlg
        dlg.add_tag.connect(self._toggle_tag_in_search)
        dlg.exclude_tag.connect(self._exclude_tag_in_search)
        dlg.search_tag.connect(self._search_only_tag)
        dlg.download.connect(lambda p: self._enqueue([p]))
        dlg.bank_toggle.connect(self._toggle_spank_bank)
        dlg.favorite_toggle.connect(self._toggle_favorite)
        dlg.upvote_toggle.connect(self._toggle_upvote)
        dlg.bank_pool_toggle.connect(self._toggle_pool_by_id_in_bank)
        dlg.view_pool.connect(lambda pid: (dlg.close(), self._open_pool_by_id(pid, new_tab=False)))
        dlg.view_pool_new_tab.connect(lambda pid: (dlg.close(), self._open_pool_by_id(pid, new_tab=True)))
        dlg.download_pool.connect(lambda pid: self._download_pool_by_id(pid))
        dlg.artist_view.connect(lambda a: (dlg.close(), self._open_artist(a, new_tab=False)))
        dlg.artist_view_new_tab.connect(lambda a: (dlg.close(), self._open_artist(a, new_tab=True)))
        dlg.artist_bank_toggle.connect(self._toggle_artist_in_bank)
        dlg.post_changed.connect(lambda p: (
            dlg.set_in_bank(self.spank_bank.has(p.id)),
            dlg.set_pool_in_bank(any(self.spank_bank.has_pool(pid) for pid in p.pools) if p.pools else False),
            dlg.update_interactions(),
            self._load_preview_image(dlg, p)
        ))
        self._load_preview_image(dlg, post)
        if start_slideshow:
            dlg.showFullScreen()
        else:
            dlg.showMaximized()
        return dlg

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
        menu.addAction("Start Slideshow from here", lambda: self._preview(post, start_slideshow=True))
        in_bank = self.spank_bank.has(post.id)
        menu.addAction("Remove from Spank Bank" if in_bank else "Save to Spank Bank",
                       lambda: self._toggle_spank_bank(post))
        menu.addAction("Remove favorite from e621" if post.is_favorited else "Favorite on e621",
                       lambda: self._toggle_favorite(post))
        menu.addAction("Remove upvote on e621" if post.own_vote == 1 else "Upvote on e621 (+1)",
                       lambda: self._toggle_upvote(post))
        menu.addAction("Download", lambda: self._enqueue([post]))
        menu.addAction("Open on e621", lambda: QDesktopServices.openUrl(QUrl(post.page_url)))
        if saved := self.history.path_for(post.id):
            if os.path.exists(saved):
                menu.addAction("Show saved file",
                               lambda: QDesktopServices.openUrl(
                                   QUrl.fromLocalFile(str(Path(saved).parent))))
        menu.addSeparator()
        for a in post.artists[:3]:
            menu.addAction(f"View artist “{a}”", lambda a=a: self._open_artist(a, new_tab=False))
            sub = menu.addMenu(f"More for artist “{a}”")
            sub.addAction("Open artist in new tab", lambda a=a: self._open_artist(a, new_tab=True))
            sub.addAction("Remove artist from Spank Bank" if self.spank_bank.has_artist(a)
                          else "Save artist to Spank Bank",
                          lambda a=a: self._toggle_artist_in_bank(a))
            sub.addAction("Open artist page on e621",
                          lambda a=a: QDesktopServices.openUrl(QUrl(artist_page_url(a))))
            sub.addSeparator()
            sub.addAction(f"Add “{a}” to search", lambda a=a: self._toggle_tag_in_search(a))
            sub.addAction(f"Search ONLY “{a}”", lambda a=a: self._search_only_tag(a))
        for pid in post.pools[:3]:
            menu.addAction(f"Open pool #{pid}", lambda pid=pid: self._open_pool_by_id(pid, new_tab=False))
            menu.addAction(f"Open pool #{pid} in new tab", lambda pid=pid: self._open_pool_by_id(pid, new_tab=True))
            pool_in_bank = self.spank_bank.has_pool(pid)
            menu.addAction(f"Remove pool #{pid} from Spank Bank" if pool_in_bank else f"Save pool #{pid} to Spank Bank",
                           lambda pid=pid: self._toggle_pool_by_id_in_bank(pid))
            menu.addAction(f"Download pool #{pid}", lambda pid=pid: self._download_pool_by_id(pid))
        menu.exec(self.grid.viewport().mapToGlobal(pos))
