"""Reusable Qt widgets."""
from __future__ import annotations

import html
import re
from urllib.parse import quote, unquote

from PySide6.QtCore import QEvent, QRect, QSize, Qt, QTimer, QUrl, Signal
from PySide6.QtGui import (QColor, QCursor, QDesktopServices, QFont, QIcon, QImage, QPainter,
                           QPixmap, QStandardItem, QStandardItemModel)
from PySide6.QtWidgets import (QAbstractItemView, QApplication, QCheckBox, QComboBox, QCompleter, QDialog,
                               QDialogButtonBox, QFormLayout, QHBoxLayout, QLabel, QLayout,
                               QLineEdit, QListView, QListWidget, QListWidgetItem,
                               QMenu, QPlainTextEdit, QPushButton, QSpinBox, QSplitter,
                               QVBoxLayout, QWidget)

from .api import BASE_URL, NON_ARTIST_TAGS, TAG_CATEGORIES, Post, Tag
from .settings import Settings

CATEGORY_COLORS = {
    0: "#5b9bd5", 1: "#e6a000", 3: "#c23bc2", 4: "#2e9e2e",
    5: "#e0601f", 6: "#d33", 7: "#888888", 8: "#2a7a2a",
}
RATING_COLORS = {"s": "#3a9e3a", "q": "#d9a400", "e": "#d23b3b"}
TILE = 150
ARTIST_LINK_SCHEME = "artist:"


def artist_page_url(name: str) -> str:
    """e621 artist wiki page (works for both registered and unregistered artists)."""
    return f"{BASE_URL}/artists/show_or_new?name={quote(name)}"


def artist_links_html(artists: list[str], color: str = "#e6a000") -> str:
    """Render artist names as clickable ``artist:<name>`` rich-text links."""
    if not artists:
        return "unknown"
    return ", ".join(
        f"<a href='{ARTIST_LINK_SCHEME}{quote(a)}' style='color:{color}; text-decoration:none;'>"
        f"{html.escape(a)}</a>"
        for a in artists)


def artist_from_link(link: str) -> str | None:
    if link.startswith(ARTIST_LINK_SCHEME):
        return unquote(link[len(ARTIST_LINK_SCHEME):]) or None
    return None


class ArtistActionsRow(QWidget):
    """Row of artist buttons (View / + Tab / Bank). Pops a menu when a post has several artists."""
    view_requested = Signal(str)
    view_new_tab_requested = Signal(str)
    bank_toggle_requested = Signal(str)

    def __init__(self, padding: str = "4px", parent: QWidget | None = None):
        super().__init__(parent)
        self._artists: list[str] = []
        self._banked: set[str] = set()
        self._padding = padding

        self.btn_view = QPushButton("View Artist")
        self.btn_view.setToolTip("Show all posts by this artist")
        self.btn_view.setStyleSheet(f"color: #e6a000; font-weight: bold; padding: {padding};")
        self.btn_view.clicked.connect(lambda: self._pick(self.view_requested, "View posts by “{}”"))

        self.btn_tab = QPushButton("+ Tab")
        self.btn_tab.setToolTip("Open this artist's posts in a new tab")
        self.btn_tab.setStyleSheet(f"color: #e6a000; font-weight: bold; padding: {padding};")
        self.btn_tab.clicked.connect(lambda: self._pick(self.view_new_tab_requested, "Open “{}” in new tab"))

        self.btn_bank = QPushButton("+ Bank Artist")
        self.btn_bank.clicked.connect(self._on_bank_clicked)

        lay = QHBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.addWidget(self.btn_view, 1)
        lay.addWidget(self.btn_tab)
        lay.addWidget(self.btn_bank)
        self.setVisible(False)

    def set_artists(self, artists: list[str], banked: set[str] | None = None) -> None:
        self._artists = list(artists)
        self._banked = set(banked or ())
        self.setVisible(bool(self._artists))
        if len(self._artists) == 1:
            self.btn_view.setText("View Artist")
            self.btn_view.setToolTip(f"Show all posts by “{self._artists[0]}”")
        else:
            self.btn_view.setText("View Artist…")
            self.btn_view.setToolTip("Show all posts by one of this post's artists")
        self._update_bank_button()

    def set_banked(self, banked: set[str]) -> None:
        self._banked = set(banked)
        self._update_bank_button()

    def _update_bank_button(self) -> None:
        many = len(self._artists) > 1
        any_in = any(a in self._banked for a in self._artists)
        if any_in:
            self.btn_bank.setText("Artist In Bank…" if many else "Artist In Bank")
            self.btn_bank.setStyleSheet(f"color: #ffffff; background-color: #d97706; font-weight: bold; "
                                        f"padding: {self._padding}; border-radius: 4px;")
            self.btn_bank.setToolTip("Artist is saved in Spank Bank (click to remove)")
        else:
            self.btn_bank.setText("+ Bank Artist…" if many else "+ Bank Artist")
            self.btn_bank.setStyleSheet(f"color: #fbbf24; font-weight: bold; padding: {self._padding};")
            self.btn_bank.setToolTip("Save artist to Spank Bank")

    def _pick(self, signal, label: str) -> None:
        if not self._artists:
            return
        if len(self._artists) == 1:
            signal.emit(self._artists[0])
            return
        menu = QMenu(self)
        for a in self._artists:
            menu.addAction(label.format(a), lambda a=a: signal.emit(a))
        menu.exec(QCursor.pos())

    def _on_bank_clicked(self) -> None:
        if not self._artists:
            return
        if len(self._artists) == 1:
            self.bank_toggle_requested.emit(self._artists[0])
            return
        menu = QMenu(self)
        for a in self._artists:
            text = f"Remove “{a}” from Spank Bank" if a in self._banked else f"Save “{a}” to Spank Bank"
            menu.addAction(text, lambda a=a: self.bank_toggle_requested.emit(a))
        menu.exec(QCursor.pos())


# -------------------------------------------------------- collapsible sections
class CollapsibleSection(QWidget):
    """Collapsible panel with a clickable header toggle button."""
    toggled = Signal(bool)

    def __init__(self, title: str, expanded: bool = False, badge_text: str = "",
                 parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._title = title
        self._expanded = expanded
        self._badge_text = badge_text

        main_lay = QVBoxLayout(self)
        main_lay.setContentsMargins(0, 3, 0, 3)
        main_lay.setSpacing(0)

        self._header = QPushButton()
        self._header.setCursor(Qt.CursorShape.PointingHandCursor)
        self._header.setCheckable(True)
        self._header.setChecked(self._expanded)
        self._header.setStyleSheet("""
            QPushButton {
                text-align: left;
                font-weight: bold;
                padding: 6px 10px;
                background-color: #21252b;
                border: 1px solid #3b4048;
                border-radius: 4px;
                color: #d7dae0;
            }
            QPushButton:hover {
                background-color: #282c34;
                border-color: #528bff;
            }
            QPushButton:checked {
                border-bottom-left-radius: 0px;
                border-bottom-right-radius: 0px;
                background-color: #252931;
            }
        """)
        self._header.clicked.connect(self._on_header_clicked)

        self._body = QWidget()
        self._body.setObjectName("collapsibleBody")
        self._body.setStyleSheet("""
            QWidget#collapsibleBody {
                background-color: #1a1d23;
                border: 1px solid #3b4048;
                border-top: none;
                border-bottom-left-radius: 4px;
                border-bottom-right-radius: 4px;
            }
            QCheckBox {
                color: #d7dae0;
                spacing: 5px;
                background: transparent;
            }
            QLabel {
                color: #9da5b4;
                background: transparent;
            }
            QPushButton {
                padding: 4px 8px;
            }
        """)
        self._body_lay = QVBoxLayout(self._body)
        self._body_lay.setContentsMargins(8, 8, 8, 8)
        self._body_lay.setSpacing(6)

        main_lay.addWidget(self._header)
        main_lay.addWidget(self._body)

        self._update_text()
        self._body.setVisible(self._expanded)

    def _update_text(self) -> None:
        prefix = "[-] " if self._expanded else "[+] "
        badge = f"  [{self._badge_text}]" if self._badge_text else ""
        self._header.setText(f"{prefix}{self._title}{badge}")

    def _on_header_clicked(self) -> None:
        self.set_expanded(self._header.isChecked())

    def set_expanded(self, expanded: bool) -> None:
        self._expanded = bool(expanded)
        self._header.setChecked(self._expanded)
        self._body.setVisible(self._expanded)
        self._update_text()
        self.toggled.emit(self._expanded)

    def is_expanded(self) -> bool:
        return self._expanded

    def set_badge(self, text: str) -> None:
        self._badge_text = text
        self._update_text()

    @property
    def body_layout(self) -> QVBoxLayout:
        return self._body_lay

    def add_widget(self, w: QWidget) -> None:
        self._body_lay.addWidget(w)

    def add_layout(self, lay: QLayout) -> None:
        self._body_lay.addLayout(lay)


# --------------------------------------------------------------------- tags
SEED_TAGS: list[Tag] = [
    # General (0)
    Tag("solo", 0, 4200000), Tag("male", 0, 3100000), Tag("female", 0, 2900000),
    Tag("anthro", 0, 3500000), Tag("feral", 0, 1100000), Tag("duo", 0, 1500000),
    Tag("fur", 0, 2400000), Tag("tail", 0, 2100000), Tag("claws", 0, 800000),
    Tag("fluffy", 0, 450000), Tag("fluffy_tail", 0, 300000), Tag("digital_media_(artwork)", 0, 3200000),
    Tag("hi_res", 0, 2800000), Tag("simple_background", 0, 1800000), Tag("detailed_background", 0, 750000),
    Tag("looking_at_viewer", 0, 1600000), Tag("smile", 0, 1200000), Tag("open_mouth", 0, 1000000),
    Tag("clothing", 0, 1400000), Tag("nude", 0, 1300000), Tag("erection", 0, 950000),
    Tag("penis", 0, 1600000), Tag("breasts", 0, 1300000), Tag("balls", 0, 1100000),
    Tag("butt", 0, 1200000), Tag("pussy", 0, 750000), Tag("anus", 0, 850000),
    Tag("canine_penis", 0, 210000), Tag("knot", 0, 450000), Tag("muscular", 0, 520000),
    Tag("spread_legs", 0, 430000), Tag("cum", 0, 980000), Tag("order:score", 0, 0),
    Tag("order:favcount", 0, 0), Tag("order:id_desc", 0, 0), Tag("comic", 0, 400000),

    # Species (5)
    Tag("canine", 5, 1700000), Tag("wolf", 5, 430000), Tag("fox", 5, 540000),
    Tag("dog", 5, 380000), Tag("feline", 5, 1200000), Tag("cat", 5, 360000),
    Tag("dragon", 5, 480000), Tag("mammal", 5, 2900000), Tag("hybrid", 5, 220000),
    Tag("pokemon_(species)", 5, 800000), Tag("rabbit", 5, 170000), Tag("hare", 5, 60000),
    Tag("equine", 5, 290000), Tag("horse", 5, 190000), Tag("deer", 5, 120000),
    Tag("bovine", 5, 150000), Tag("otter", 5, 110000), Tag("mustelid", 5, 180000),
    Tag("bear", 5, 110000), Tag("ursine", 5, 130000), Tag("raccoon", 5, 75000),
    Tag("procyonid", 5, 85000), Tag("shark", 5, 85000), Tag("reptile", 5, 450000),
    Tag("scalie", 5, 350000), Tag("avian", 5, 140000), Tag("bird", 5, 110000),
    Tag("hyena", 5, 100000), Tag("rodent", 5, 160000), Tag("mouse", 5, 80000),
    Tag("tiger", 5, 120000), Tag("lion", 5, 130000), Tag("leopard", 5, 70000),
    Tag("cheetah", 5, 55000), Tag("big_cat", 5, 160000),

    # Copyright (3)
    Tag("pokemon", 3, 900000), Tag("nintendo", 3, 1100000), Tag("digimon", 3, 180000),
    Tag("disney", 3, 150000), Tag("my_little_pony", 3, 160000), Tag("hasbro", 3, 170000),
    Tag("sega", 3, 140000), Tag("sonic_the_hedgehog", 3, 130000), Tag("capcom", 3, 80000),
    Tag("zootopia", 3, 75000), Tag("star_fox", 3, 70000),

    # Character (4)
    Tag("lucario", 4, 95000), Tag("renamon", 4, 45000), Tag("judy_hopps", 4, 30000),
    Tag("nick_wilde", 4, 35000), Tag("krystal_(star_fox)", 4, 25000),
    Tag("wolf_o'donnell", 4, 8000), Tag("tails_(sonic)", 4, 25000),
    Tag("sonic_the_hedgehog_(character)", 4, 30000),

    # Artist (1)
    Tag("conditional_dnp", 1, 150000), Tag("unknown_artist", 1, 120000),
]


class TagLineEdit(QLineEdit):
    """Line edit with e621 tag autocomplete on the token under the cursor.
    Appends cleanly and never overwrites other tags in the search query.
    """

    suggestions_wanted = Signal(str)

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent)
        self.setPlaceholderText("e.g. wolf solo -comic order:score")
        self._known_tags: dict[str, Tag] = {t.name: t for t in SEED_TAGS}
        self._timer = QTimer(self, singleShot=True, interval=150)
        self._timer.timeout.connect(self._request)
        self.textEdited.connect(self._on_text_edited)

        self._popup = QListWidget()
        self._popup.setWindowFlags(Qt.WindowType.ToolTip | Qt.WindowType.FramelessWindowHint)
        self._popup.setFocusPolicy(Qt.FocusPolicy.NoFocus)
        self._popup.itemClicked.connect(self._on_item_clicked)
        self._popup.setStyleSheet("""
            QListWidget {
                background: #1e222b;
                color: #e0e0e0;
                border: 1px solid #5b9bd5;
                border-radius: 4px;
                padding: 2px;
                font-size: 12px;
            }
            QListWidget::item {
                padding: 4px 8px;
                border-radius: 3px;
            }
            QListWidget::item:hover, QListWidget::item:selected {
                background: #2b3342;
            }
        """)

    def learn_tags(self, tags: list[Tag] | list[str]) -> None:
        for t in tags:
            if isinstance(t, Tag):
                self._known_tags[t.name] = t
            elif isinstance(t, str) and t not in self._known_tags:
                self._known_tags[t] = Tag(t, 0, 0)

    def learn_post_tags(self, post: Post) -> None:
        try:
            cat_map = {"general": 0, "artist": 1, "copyright": 3, "character": 4, "species": 5}
            for cat_name, names in (post.tags or {}).items():
                cat_id = cat_map.get(cat_name, 0)
                for name in names:
                    if name not in self._known_tags:
                        self._known_tags[name] = Tag(name, cat_id, 0)
        except Exception:
            pass

    def current_token(self) -> str:
        before = self.text()[: self.cursorPosition()]
        tok = before.rsplit(" ", 1)[-1]
        return tok.lstrip("-~").lower()

    def _on_text_edited(self, _text: str) -> None:
        tok = self.current_token()
        if len(tok) >= 2 and ":" not in tok:
            # Instant local matching from known/seed tags
            matches = self._find_local_matches(tok)
            if matches:
                self._render_popup(matches[:15])
            self._timer.start()
        else:
            self._popup.hide()

    def _find_local_matches(self, tok: str) -> list[Tag]:
        starts = [t for t in self._known_tags.values() if t.name.startswith(tok)]
        starts.sort(key=lambda t: t.post_count, reverse=True)
        return starts

    def _request(self) -> None:
        tok = self.current_token()
        if len(tok) >= 2 and ":" not in tok:
            self.suggestions_wanted.emit(tok)
        else:
            self._popup.hide()

    def set_suggestions(self, token: str, tags: list[Tag]) -> None:
        for t in tags:
            self._known_tags[t.name] = t

        cur = self.current_token()
        if len(cur) < 2 or ":" in cur:
            self._popup.hide()
            return

        matches = self._find_local_matches(cur)
        if matches:
            self._render_popup(matches[:15])
        else:
            self._popup.hide()

    def _render_popup(self, tags: list[Tag]) -> None:
        if not tags or not self.isVisible():
            self._popup.hide()
            return

        self._popup.clear()
        for t in tags:
            cnt_str = f"({t.post_count:,})" if t.post_count > 0 else ""
            item = QListWidgetItem(f"{t.name:<25} {cnt_str:>12} · {t.category_name}")
            item.setData(Qt.ItemDataRole.UserRole, t.name)
            item.setForeground(QColor(CATEGORY_COLORS.get(t.category, "#5b9bd5")))
            self._popup.addItem(item)

        self._popup.setCurrentRow(0)
        p = self.mapToGlobal(self.rect().bottomLeft())
        w = max(self.width(), 380)
        row_h = self._popup.sizeHintForRow(0) if self._popup.sizeHintForRow(0) > 0 else 24
        h = min(260, row_h * len(tags) + 8)
        self._popup.setGeometry(p.x(), p.y() + 2, w, h)
        self._popup.show()

    def _on_item_clicked(self, item: QListWidgetItem) -> None:
        tag_name = item.data(Qt.ItemDataRole.UserRole)
        if tag_name:
            self._insert_tag(tag_name, add_space=False)
        self._popup.hide()

    def _insert_tag(self, name: str, add_space: bool = False) -> None:
        full = self.text()
        pos = self.cursorPosition()
        before = full[:pos]
        after = full[pos:]
        start = before.rfind(" ") + 1
        tok = before[start:]
        prefix = tok[0] if tok[:1] in ("-", "~") else ""
        end_match = re.match(r"\S*", after)
        end_len = end_match.end() if end_match else 0
        suffix = " " if add_space else ""
        new_before = before[:start] + prefix + name + suffix
        new_after = after[end_len:].lstrip()
        self.setText(new_before + new_after)
        self.setCursorPosition(len(new_before))
        self.setFocus()

    def keyPressEvent(self, e):  # noqa: N802
        if self._popup.isVisible():
            if e.key() in (Qt.Key.Key_Down, Qt.Key.Key_Up):
                cur = self._popup.currentRow()
                delta = 1 if e.key() == Qt.Key.Key_Down else -1
                self._popup.setCurrentRow(max(0, min(self._popup.count() - 1, cur + delta)))
                return
            if e.key() in (Qt.Key.Key_Return, Qt.Key.Key_Enter):
                it = self._popup.currentItem()
                if it:
                    tag_name = it.data(Qt.ItemDataRole.UserRole)
                    if tag_name:
                        self._insert_tag(tag_name, add_space=False)
                self._popup.hide()
                self.returnPressed.emit()
                return
            if e.key() == Qt.Key.Key_Tab:
                it = self._popup.currentItem()
                if it:
                    tag_name = it.data(Qt.ItemDataRole.UserRole)
                    if tag_name:
                        self._insert_tag(tag_name, add_space=True)
                self._popup.hide()
                return
            if e.key() == Qt.Key.Key_Escape:
                self._popup.hide()
                return
        super().keyPressEvent(e)

    def focusOutEvent(self, e):  # noqa: N802
        super().focusOutEvent(e)
        QTimer.singleShot(150, self._check_focus_out)

    def _check_focus_out(self) -> None:
        if not self.hasFocus() and not self._popup.underMouse():
            self._popup.hide()

    def hideEvent(self, e):  # noqa: N802
        try:
            if hasattr(self, "_popup") and self._popup is not None:
                self._popup.hide()
        except RuntimeError:
            pass
        super().hideEvent(e)


# ------------------------------------------------------------------ hover preview
class HoverPreviewPopup(QWidget):
    """Floating medium-sized preview shown when hovering over a thumbnail."""

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent, Qt.WindowType.ToolTip | Qt.WindowType.FramelessWindowHint)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating)
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self.setStyleSheet("""
            QWidget {
                background-color: #1a1d24;
                border: 2px solid #5b9bd5;
                border-radius: 6px;
                color: #e2e8f0;
            }
        """)
        self._image_label = QLabel()
        self._image_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._image_label.setFixedSize(380, 320)
        self._image_label.setStyleSheet("border: none; background-color: #12141a; border-radius: 4px;")

        self._info_label = QLabel()
        self._info_label.setWordWrap(True)
        self._info_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._info_label.setStyleSheet("border: none; font-size: 11px; padding: 4px; color: #cbd5e1;")

        lay = QVBoxLayout(self)
        lay.setContentsMargins(6, 6, 6, 6)
        lay.setSpacing(4)
        lay.addWidget(self._image_label)
        lay.addWidget(self._info_label)

    def show_preview(self, post: Post, image: QImage | None, global_pos) -> None:
        if image and not image.isNull():
            pm = QPixmap.fromImage(image).scaled(
                self._image_label.size(),
                Qt.AspectRatioMode.KeepAspectRatio,
                Qt.TransformationMode.SmoothTransformation,
            )
            self._image_label.setPixmap(pm)
        else:
            self._image_label.setText("No preview image" if not post.preview_url else "Loading…")

        artists = ", ".join(post.artists) or "unknown"
        mb = post.size / 1_048_576 if post.size else 0
        details = [f"<b>#{post.id}</b>", post.ext.upper(), f"{post.width}×{post.height}"]
        if mb > 0:
            details.append(f"{mb:.1f} MB")
        details.append(f"Score {post.score}")
        if post.rating:
            details.append(f"Rating {post.rating.upper()}")

        self._info_label.setText(
            f"{' · '.join(details)}<br><span style='color:#a0aec0;'>Artist: {artists}</span>"
        )
        self.adjustSize()

        screen = QApplication.primaryScreen().availableGeometry()
        x = global_pos.x() + 24
        y = global_pos.y() - self.height() // 2
        if x + self.width() > screen.right() - 10:
            x = global_pos.x() - self.width() - 24
        if y + self.height() > screen.bottom() - 10:
            y = screen.bottom() - self.height() - 10
        if y < screen.top() + 10:
            y = screen.top() + 10
        self.move(x, y)
        self.show()


# --------------------------------------------------------------------- grid
def render_tile(image: QImage | None, post: Post, downloaded: bool,
                placeholder: str = "loading…") -> QIcon:
    pm = QPixmap(TILE, TILE)
    pm.fill(Qt.GlobalColor.transparent)
    p = QPainter(pm)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    if image is not None and not image.isNull():
        img = image.scaled(TILE, TILE, Qt.AspectRatioMode.KeepAspectRatio,
                           Qt.TransformationMode.SmoothTransformation)
        p.drawImage((TILE - img.width()) // 2, (TILE - img.height()) // 2, img)
    else:
        p.fillRect(pm.rect(), QColor("#2b303b"))
        p.setPen(QColor("#9aa"))
        p.drawText(pm.rect(), Qt.AlignmentFlag.AlignCenter, placeholder)

    font = QFont()
    font.setBold(True)
    font.setPointSize(8)
    p.setFont(font)

    def badge(x: int, y: int, text: str, color: str) -> None:
        w = p.fontMetrics().horizontalAdvance(text) + 8
        r = QRect(x if x >= 0 else TILE + x - w, y, w, 16)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(QColor(color))
        p.drawRoundedRect(r, 4, 4)
        p.setPen(QColor("white"))
        p.drawText(r, Qt.AlignmentFlag.AlignCenter, text)

    if post.rating:
        badge(2, 2, post.rating.upper(), RATING_COLORS.get(post.rating, "#666"))
    if post.ext in ("webm", "mp4", "gif", "swf"):
        badge(2, TILE - 18, post.ext.upper(), "#334")
    if post.pools:
        badge(-2, TILE - 18, "POOL", "#5a3e9e")
    if downloaded:
        badge(-2, 2, "SAVED", "#1f8f3a")
    p.end()
    return QIcon(pm)


class PostGrid(QListWidget):
    near_bottom = Signal()
    post_clicked = Signal(object)

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent)
        self.setViewMode(QListView.ViewMode.IconMode)
        self.setIconSize(QSize(TILE, TILE))
        self.setGridSize(QSize(TILE + 18, TILE + 34))
        self.setResizeMode(QListView.ResizeMode.Adjust)
        self.setMovement(QListView.Movement.Static)
        self.setUniformItemSizes(True)
        self.setSelectionMode(QAbstractItemView.SelectionMode.ExtendedSelection)
        self.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.setMouseTracking(True)
        self.verticalScrollBar().valueChanged.connect(self._check_scroll)
        self._posts: dict[int, Post] = {}
        self._items: dict[int, QListWidgetItem] = {}
        self._images: dict[int, QImage] = {}
        self._downloaded: set[int] = set()
        self._hover_popup = HoverPreviewPopup(self)

    def clear_posts(self) -> None:
        self._hover_popup.hide()
        self.clear()
        self._posts.clear()
        self._items.clear()
        self._images.clear()
        self._downloaded.clear()

    def add_posts(self, posts: list[Post], downloaded: set[int]) -> list[Post]:
        added = []
        for post in posts:
            if post.id in self._posts:
                continue
            is_dl = post.id in downloaded
            if is_dl:
                self._downloaded.add(post.id)
            item = QListWidgetItem(render_tile(None, post, is_dl,
                                               "loading…" if post.preview_url else "no preview\n(login?)"),
                                   f"#{post.id}  Score {post.score}")
            item.setData(Qt.ItemDataRole.UserRole, post.id)
            item.setToolTip(self._tooltip(post))
            self.addItem(item)
            self._posts[post.id] = post
            self._items[post.id] = item
            added.append(post)
        return added

    @staticmethod
    def _tooltip(post: Post) -> str:
        artists = ", ".join(post.artists) or "unknown"
        mb = post.size / 1_048_576
        return (f"#{post.id} · {post.ext.upper()} · {post.width}×{post.height} · {mb:.1f} MB\n"
                f"Artist: {artists}\nScore {post.score} · Favs {post.fav_count} · Rating {post.rating}\n"
                "Click for full screen view")

    def set_thumbnail(self, post_id: int, image: QImage) -> None:
        item = self._items.get(post_id)
        if item is None:
            return
        self._images[post_id] = image
        item.setIcon(render_tile(image, self._posts[post_id], post_id in self._downloaded,
                                 "no preview"))

    def mark_downloaded(self, post_id: int) -> None:
        if post_id in self._downloaded or post_id not in self._items:
            return
        self._downloaded.add(post_id)
        self._items[post_id].setIcon(render_tile(self._images.get(post_id), self._posts[post_id],
                                                 True, "no preview"))

    def post_for(self, item: QListWidgetItem) -> Post:
        return self._posts[item.data(Qt.ItemDataRole.UserRole)]

    def selected_posts(self) -> list[Post]:
        rows = sorted(self.row(i) for i in self.selectedItems())
        return [self.post_for(self.item(r)) for r in rows]

    def all_posts(self) -> list[Post]:
        return [self.post_for(self.item(r)) for r in range(self.count())]

    def is_scrollable(self) -> bool:
        return self.verticalScrollBar().maximum() > 0

    def _check_scroll(self, value: int) -> None:
        self._hover_popup.hide()
        sb = self.verticalScrollBar()
        if sb.maximum() > 0 and value >= sb.maximum() - TILE * 2:
            self.near_bottom.emit()

    def mouseMoveEvent(self, e):  # noqa: N802
        super().mouseMoveEvent(e)
        item = self.itemAt(e.pos())
        if item is not None:
            post_id = item.data(Qt.ItemDataRole.UserRole)
            post = self._posts.get(post_id)
            if post:
                img = self._images.get(post_id)
                self._hover_popup.show_preview(post, img, e.globalPosition().toPoint())
                return
        self._hover_popup.hide()

    def leaveEvent(self, e):  # noqa: N802
        self._hover_popup.hide()
        super().leaveEvent(e)

    def mousePressEvent(self, e):  # noqa: N802
        self._hover_popup.hide()
        super().mousePressEvent(e)
        if e.button() == Qt.MouseButton.LeftButton:
            modifiers = QApplication.keyboardModifiers()
            if not (modifiers & (Qt.KeyboardModifier.ControlModifier | Qt.KeyboardModifier.ShiftModifier)):
                item = self.itemAt(e.pos())
                if item:
                    post = self.post_for(item)
                    self.post_clicked.emit(post)

    def wheelEvent(self, e):  # noqa: N802
        self._hover_popup.hide()
        super().wheelEvent(e)


# ------------------------------------------------------------------ tag list
class TagListWidget(QListWidget):
    tag_clicked = Signal(str)           # emitted on single click (toggle/add)
    tag_exclude_requested = Signal(str) # right-click -> exclude (-tag)
    tag_search_requested = Signal(str)  # right-click -> search only this tag
    artist_view_requested = Signal(str)          # right-click on artist -> view artist's posts
    artist_view_new_tab_requested = Signal(str)  # right-click on artist -> view in new tab
    artist_bank_toggle_requested = Signal(str)   # right-click on artist -> save/remove in Spank Bank

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent)
        self.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.customContextMenuRequested.connect(self._show_context_menu)
        self.itemClicked.connect(self._on_item_clicked)
        self._raw_tags: dict[str, list[str]] = {}
        self._active_search_tokens: set[str] = set()
        self.artist_in_bank = lambda _name: False  # injected by owner for menu labels

    def set_post_tags(self, tags: dict[str, list[str]], active_query: str = "") -> None:
        self.clear()
        self._raw_tags = tags
        self._active_search_tokens = set(active_query.split())

        category_order = ("artist", "copyright", "character", "species", "general", "lore", "meta")
        for cat in category_order:
            cat_id = next((k for k, v in TAG_CATEGORIES.items() if v == cat), 0)
            tag_list = tags.get(cat, [])
            for t in sorted(tag_list):
                is_inc = t in self._active_search_tokens
                is_exc = f"-{t}" in self._active_search_tokens
                prefix = "[+] " if is_inc else ("[-] " if is_exc else "")

                it = QListWidgetItem(f"{prefix}{t}")
                it.setData(Qt.ItemDataRole.UserRole, t)
                it.setData(Qt.ItemDataRole.UserRole + 1, cat)
                color = QColor(CATEGORY_COLORS.get(cat_id, "#5b9bd5"))
                it.setForeground(color)
                f = it.font()
                f.setBold(is_inc or is_exc)
                it.setFont(f)
                it.setToolTip(f"[{cat.capitalize()}] {t}\nClick to add/remove from search\nRight-click for more options")
                self.addItem(it)

    def refresh_active_tags(self, active_query: str) -> None:
        self._active_search_tokens = set(active_query.split())
        for i in range(self.count()):
            it = self.item(i)
            t = it.data(Qt.ItemDataRole.UserRole)
            if not t:
                continue
            is_inc = t in self._active_search_tokens
            is_exc = f"-{t}" in self._active_search_tokens
            prefix = "[+] " if is_inc else ("[-] " if is_exc else "")
            it.setText(f"{prefix}{t}")
            f = it.font()
            f.setBold(is_inc or is_exc)
            it.setFont(f)

    def filter_text(self, text: str) -> None:
        q = text.strip().lower()
        for i in range(self.count()):
            it = self.item(i)
            t = (it.data(Qt.ItemDataRole.UserRole) or "").lower()
            it.setHidden(bool(q and q not in t))

    def _on_item_clicked(self, it: QListWidgetItem) -> None:
        tag = it.data(Qt.ItemDataRole.UserRole)
        if tag:
            self.tag_clicked.emit(tag)

    def _show_context_menu(self, pos) -> None:
        it = self.itemAt(pos)
        if not it:
            return
        tag = it.data(Qt.ItemDataRole.UserRole)
        if not tag:
            return
        menu = QMenu(self)
        is_inc = tag in self._active_search_tokens
        is_exc = f"-{tag}" in self._active_search_tokens

        if it.data(Qt.ItemDataRole.UserRole + 1) == "artist" and tag not in NON_ARTIST_TAGS:
            menu.addAction(f"View artist “{tag}”", lambda: self.artist_view_requested.emit(tag))
            menu.addAction("Open artist in new tab", lambda: self.artist_view_new_tab_requested.emit(tag))
            in_bank = self.artist_in_bank(tag)
            menu.addAction("Remove artist from Spank Bank" if in_bank else "Save artist to Spank Bank",
                           lambda: self.artist_bank_toggle_requested.emit(tag))
            menu.addAction("Open artist page on e621",
                           lambda: QDesktopServices.openUrl(QUrl(artist_page_url(tag))))
            menu.addSeparator()

        if is_inc:
            menu.addAction(f"Remove “{tag}” from search", lambda: self.tag_clicked.emit(tag))
        else:
            menu.addAction(f"Add to search (+{tag})", lambda: self.tag_clicked.emit(tag))

        if is_exc:
            menu.addAction(f"Remove exclusion (-{tag})", lambda: self.tag_exclude_requested.emit(tag))
        else:
            menu.addAction(f"Exclude from search (-{tag})", lambda: self.tag_exclude_requested.emit(tag))

        menu.addSeparator()
        menu.addAction(f"Search only “{tag}”", lambda: self.tag_search_requested.emit(tag))
        menu.addAction("Copy tag name", lambda: QApplication.clipboard().setText(tag))
        menu.exec(self.viewport().mapToGlobal(pos))


# ------------------------------------------------------------------ inspector
class PostInspector(QWidget):
    tag_clicked = Signal(str)
    tag_exclude_requested = Signal(str)
    tag_search_requested = Signal(str)
    preview_requested = Signal(object)
    download_requested = Signal(object)
    bank_toggle_requested = Signal(object)
    favorite_toggle_requested = Signal(object)
    upvote_toggle_requested = Signal(object)
    view_pool_requested = Signal(int)
    view_pool_new_tab_requested = Signal(int)
    bank_pool_requested = Signal(int)
    download_pool_requested = Signal(int)
    artist_view_requested = Signal(str)
    artist_view_new_tab_requested = Signal(str)
    artist_bank_toggle_requested = Signal(str)

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent)
        self._current_post: Post | None = None

        self.title_label = QLabel("<b>No post selected</b>")
        self.title_label.setWordWrap(True)
        self.title_label.setTextFormat(Qt.TextFormat.RichText)
        self.title_label.setTextInteractionFlags(Qt.TextInteractionFlag.LinksAccessibleByMouse)
        self.title_label.linkActivated.connect(self._on_title_link)
        self.title_label.setToolTip("Click an artist name to view all of their posts")

        self.thumb_label = QLabel()
        self.thumb_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.thumb_label.setFixedHeight(140)
        self.thumb_label.setStyleSheet("background: #252830; border-radius: 4px; color: #778;")
        self.thumb_label.setText("Select a post")

        self.info_label = QLabel("Click an image in the grid to view tags.")
        self.info_label.setWordWrap(True)
        self.info_label.setStyleSheet("color: #888;")

        btn_row = QHBoxLayout()
        self.btn_preview = QPushButton("Preview")
        self.btn_preview.setEnabled(False)
        self.btn_preview.clicked.connect(self._emit_preview)

        self.btn_bank = QPushButton("+ Bank")
        self.btn_bank.setEnabled(False)
        self.btn_bank.setToolTip("Save/remove from Spank Bank for online viewing")
        self.btn_bank.clicked.connect(self._emit_bank_toggle)

        self.btn_download = QPushButton("Download")
        self.btn_download.setEnabled(False)
        self.btn_download.clicked.connect(self._emit_download)

        btn_row.addWidget(self.btn_preview)
        btn_row.addWidget(self.btn_bank)
        btn_row.addWidget(self.btn_download)

        # e621 Site Interactions (Favorite / Upvote)
        self.btn_fav = QPushButton("Favorite")
        self.btn_fav.setEnabled(False)
        self.btn_fav.setToolTip("Like and favorite on e621 (credits the artist)")
        self.btn_fav.setStyleSheet("font-weight: bold; padding: 4px;")
        self.btn_fav.clicked.connect(self._emit_favorite_toggle)

        self.btn_upvote = QPushButton("▲ +1")
        self.btn_upvote.setEnabled(False)
        self.btn_upvote.setToolTip("Upvote on e621 (increases post score)")
        self.btn_upvote.setStyleSheet("font-weight: bold; padding: 4px;")
        self.btn_upvote.clicked.connect(self._emit_upvote_toggle)

        interact_row = QHBoxLayout()
        interact_row.addWidget(self.btn_fav, 1)
        interact_row.addWidget(self.btn_upvote)

        # Pool actions
        self.btn_view_pool = QPushButton("View Pool")
        self.btn_view_pool.setStyleSheet("color: #a78bfa; font-weight: bold;")
        self.btn_view_pool.clicked.connect(self._emit_view_pool)

        self.btn_tab_pool = QPushButton("+ Tab")
        self.btn_tab_pool.setToolTip("Open pool in a new tab")
        self.btn_tab_pool.setStyleSheet("color: #a78bfa; font-weight: bold;")
        self.btn_tab_pool.clicked.connect(self._emit_view_pool_new_tab)

        self.btn_bank_pool = QPushButton("+ Bank")
        self.btn_bank_pool.setToolTip("Save/remove pool from Spank Bank")
        self.btn_bank_pool.setStyleSheet("color: #fbbf24; font-weight: bold;")
        self.btn_bank_pool.clicked.connect(self._emit_bank_pool)

        self.btn_download_pool = QPushButton("Download Pool")
        self.btn_download_pool.setStyleSheet("color: #60a5fa; font-weight: bold;")
        self.btn_download_pool.clicked.connect(self._emit_download_pool)

        pool_row = QHBoxLayout()
        pool_row.addWidget(self.btn_view_pool)
        pool_row.addWidget(self.btn_tab_pool)
        pool_row.addWidget(self.btn_bank_pool)
        pool_row.addWidget(self.btn_download_pool)
        self.pool_widget = QWidget()
        self.pool_widget.setLayout(pool_row)
        self.pool_widget.setVisible(False)

        # Artist actions
        self.artist_row = ArtistActionsRow()
        self.artist_row.view_requested.connect(self.artist_view_requested)
        self.artist_row.view_new_tab_requested.connect(self.artist_view_new_tab_requested)
        self.artist_row.bank_toggle_requested.connect(self.artist_bank_toggle_requested)

        self.filter_edit = QLineEdit()
        self.filter_edit.setPlaceholderText("Filter tags…")
        self.filter_edit.setClearButtonEnabled(True)
        self.filter_edit.textChanged.connect(self._on_filter_changed)

        self.tag_header = QLabel("Tags (click to add to search):")
        self.tag_header.setStyleSheet("font-weight: bold; margin-top: 4px;")

        self.tag_list = TagListWidget()
        self.tag_list.tag_clicked.connect(self.tag_clicked)
        self.tag_list.tag_exclude_requested.connect(self.tag_exclude_requested)
        self.tag_list.tag_search_requested.connect(self.tag_search_requested)
        self.tag_list.artist_view_requested.connect(self.artist_view_requested)
        self.tag_list.artist_view_new_tab_requested.connect(self.artist_view_new_tab_requested)
        self.tag_list.artist_bank_toggle_requested.connect(self.artist_bank_toggle_requested)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(6, 6, 6, 6)
        layout.addWidget(self.title_label)
        layout.addWidget(self.thumb_label)
        layout.addWidget(self.info_label)
        layout.addLayout(btn_row)
        layout.addLayout(interact_row)
        layout.addWidget(self.artist_row)
        layout.addWidget(self.pool_widget)
        layout.addWidget(self.tag_header)
        layout.addWidget(self.filter_edit)
        layout.addWidget(self.tag_list, 1)

    def _on_title_link(self, link: str) -> None:
        if name := artist_from_link(link):
            self.artist_view_requested.emit(name)

    def set_artists_banked(self, banked: set[str]) -> None:
        self.artist_row.set_banked(banked)

    def _emit_preview(self) -> None:
        if self._current_post:
            self.preview_requested.emit(self._current_post)

    def _emit_download(self) -> None:
        if self._current_post:
            self.download_requested.emit(self._current_post)

    def _emit_view_pool(self) -> None:
        if not self._current_post or not self._current_post.pools:
            return
        if len(self._current_post.pools) == 1:
            self.view_pool_requested.emit(self._current_post.pools[0])
        else:
            menu = QMenu(self)
            for pid in self._current_post.pools:
                menu.addAction(f"View Pool #{pid}", lambda pid=pid: self.view_pool_requested.emit(pid))
            menu.exec(QCursor.pos())

    def _emit_view_pool_new_tab(self) -> None:
        if not self._current_post or not self._current_post.pools:
            return
        if len(self._current_post.pools) == 1:
            self.view_pool_new_tab_requested.emit(self._current_post.pools[0])
        else:
            menu = QMenu(self)
            for pid in self._current_post.pools:
                menu.addAction(f"Open Pool #{pid} in new tab", lambda pid=pid: self.view_pool_new_tab_requested.emit(pid))
            menu.exec(QCursor.pos())

    def _emit_download_pool(self) -> None:
        if not self._current_post or not self._current_post.pools:
            return
        if len(self._current_post.pools) == 1:
            self.download_pool_requested.emit(self._current_post.pools[0])
        else:
            menu = QMenu(self)
            for pid in self._current_post.pools:
                menu.addAction(f"Download Pool #{pid}", lambda pid=pid: self.download_pool_requested.emit(pid))
            menu.exec(QCursor.pos())

    def _emit_bank_pool(self) -> None:
        if not self._current_post or not self._current_post.pools:
            return
        if len(self._current_post.pools) == 1:
            self.bank_pool_requested.emit(self._current_post.pools[0])
        else:
            menu = QMenu(self)
            for pid in self._current_post.pools:
                menu.addAction(f"Save/Remove Pool #{pid} in Spank Bank",
                               lambda pid=pid: self.bank_pool_requested.emit(pid))
            menu.exec(QCursor.pos())

    def set_pool_in_bank(self, in_bank: bool) -> None:
        if in_bank:
            self.btn_bank_pool.setText("In Bank")
            self.btn_bank_pool.setStyleSheet("color: #fbbf24; font-weight: bold;")
            self.btn_bank_pool.setToolTip("Pool is saved in Spank Bank (click to remove)")
        else:
            self.btn_bank_pool.setText("+ Bank")
            self.btn_bank_pool.setStyleSheet("color: #a78bfa; font-weight: bold;")
            self.btn_bank_pool.setToolTip("Save pool to Spank Bank")

    def _emit_bank_toggle(self) -> None:
        if self._current_post:
            self.bank_toggle_requested.emit(self._current_post)

    def _emit_favorite_toggle(self) -> None:
        if self._current_post:
            self.favorite_toggle_requested.emit(self._current_post)

    def _emit_upvote_toggle(self) -> None:
        if self._current_post:
            self.upvote_toggle_requested.emit(self._current_post)

    def update_interactions(self, post: Post | None = None) -> None:
        p = post or self._current_post
        if not p:
            self.btn_fav.setEnabled(False)
            self.btn_fav.setText("Favorite")
            self.btn_fav.setStyleSheet("font-weight: bold; padding: 4px;")
            self.btn_fav.setToolTip("Like and favorite on e621 (credits the artist)")
            self.btn_upvote.setEnabled(False)
            self.btn_upvote.setText("▲ +1")
            self.btn_upvote.setStyleSheet("font-weight: bold; padding: 4px;")
            self.btn_upvote.setToolTip("Upvote on e621 (increases post score)")
            return

        self.btn_fav.setEnabled(True)
        self.btn_upvote.setEnabled(True)

        if p.is_favorited:
            self.btn_fav.setText(f"Favorited ({p.fav_count})")
            self.btn_fav.setStyleSheet("color: #ffffff; background-color: #e11d48; font-weight: bold; padding: 4px; border-radius: 4px;")
            self.btn_fav.setToolTip("Favorited on e621 (click to remove favorite)")
        else:
            self.btn_fav.setText(f"Favorite ({p.fav_count})")
            self.btn_fav.setStyleSheet("color: #fda4af; font-weight: bold; padding: 4px;")
            self.btn_fav.setToolTip("Like and favorite on e621 (credits the artist)")

        if p.own_vote == 1:
            self.btn_upvote.setText(f"▲ Voted ({p.score})")
            self.btn_upvote.setStyleSheet("color: #ffffff; background-color: #16a34a; font-weight: bold; padding: 4px; border-radius: 4px;")
            self.btn_upvote.setToolTip("Upvoted on e621 (click to remove vote)")
        else:
            self.btn_upvote.setText(f"▲ +1 ({p.score})")
            self.btn_upvote.setStyleSheet("color: #86efac; font-weight: bold; padding: 4px;")
            self.btn_upvote.setToolTip("Upvote on e621 (increases post score)")

    def set_in_bank(self, in_bank: bool) -> None:
        if in_bank:
            self.btn_bank.setText("In Bank")
            self.btn_bank.setStyleSheet("color: #fbbf24; font-weight: bold;")
            self.btn_bank.setToolTip("In Spank Bank (click to remove)")
        else:
            self.btn_bank.setText("+ Bank")
            self.btn_bank.setStyleSheet("color: #94a3b8;")
            self.btn_bank.setToolTip("Save to Spank Bank for online viewing")

    def _on_filter_changed(self, text: str) -> None:
        self.tag_list.filter_text(text)

    def set_post(self, post: Post | None, thumb_image: QImage | None = None,
                 active_query: str = "", in_bank: bool = False, pool_in_bank: bool = False,
                 artists_banked: set[str] | None = None) -> None:
        self._current_post = post
        self.filter_edit.clear()
        if post is None:
            self.title_label.setText("<b>No post selected</b>")
            self.thumb_label.clear()
            self.thumb_label.setText("Select a post")
            self.info_label.setText("Click an image in the grid to view tags.")
            self.btn_preview.setEnabled(False)
            self.btn_bank.setEnabled(False)
            self.btn_download.setEnabled(False)
            self.set_in_bank(False)
            self.set_pool_in_bank(False)
            self.update_interactions(None)
            self.pool_widget.setVisible(False)
            self.artist_row.set_artists([])
            self.tag_list.clear()
            return

        self.title_label.setText(f"<b>Post #{post.id}</b> · {artist_links_html(post.artists)}")
        self.artist_row.set_artists(post.artists, artists_banked)
        self.btn_preview.setEnabled(True)
        self.btn_bank.setEnabled(True)
        self.btn_download.setEnabled(True)
        self.set_in_bank(in_bank)
        self.set_pool_in_bank(pool_in_bank)
        self.update_interactions(post)

        if post.pools:
            self.pool_widget.setVisible(True)
            if len(post.pools) == 1:
                pid = post.pools[0]
                self.btn_view_pool.setText(f"View Pool #{pid}")
                self.btn_download_pool.setText(f"Download Pool #{pid}")
            else:
                self.btn_view_pool.setText("View Pool…")
                self.btn_download_pool.setText("Download Pool…")
        else:
            self.pool_widget.setVisible(False)

        mb = post.size / 1_048_576 if post.size else 0
        details = [f"{post.ext.upper()}", f"{post.width}×{post.height}"]
        if mb > 0:
            details.append(f"{mb:.1f} MB")
        details.append(f"Score {post.score}")
        details.append(f"Favs {post.fav_count}")
        if post.rating:
            details.append(f"Rating {post.rating.upper()}")
        self.info_label.setText(" · ".join(details))

        if thumb_image and not thumb_image.isNull():
            pm = QPixmap.fromImage(thumb_image).scaled(
                240, 140, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation)
            self.thumb_label.setPixmap(pm)
        else:
            self.thumb_label.setText("No preview" if not post.preview_url else "Loading…")

        self.tag_list.set_post_tags(post.tags, active_query)

    def set_thumbnail(self, post_id: int, image: QImage) -> None:
        if self._current_post and self._current_post.id == post_id and image and not image.isNull():
            pm = QPixmap.fromImage(image).scaled(
                240, 140, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation)
            self.thumb_label.setPixmap(pm)

    def refresh_active_tags(self, active_query: str) -> None:
        self.tag_list.refresh_active_tags(active_query)


# ------------------------------------------------------------------ full screen viewer
class PreviewDialog(QDialog):
    add_tag = Signal(str)
    exclude_tag = Signal(str)
    search_tag = Signal(str)
    download = Signal(object)
    bank_toggle = Signal(object)
    favorite_toggle = Signal(object)
    upvote_toggle = Signal(object)
    post_changed = Signal(object)
    view_pool = Signal(int)
    view_pool_new_tab = Signal(int)
    bank_pool_toggle = Signal(int)
    download_pool = Signal(int)
    artist_view = Signal(str)
    artist_view_new_tab = Signal(str)
    artist_bank_toggle = Signal(str)

    def __init__(self, post: Post, all_posts: list[Post] | None = None, index: int = 0,
                 active_query: str = "", in_bank: bool = False, pool_in_bank: bool = False,
                 start_slideshow: bool = False, shuffle: bool = False,
                 artist_in_bank=None, parent: QWidget | None = None):
        super().__init__(parent)
        self._posts = all_posts if all_posts else [post]
        self._index = max(0, min(len(self._posts) - 1, index))
        self.post = self._posts[self._index]
        self._active_query = active_query
        self._in_bank = in_bank
        self._pool_in_bank = pool_in_bank
        self._artist_in_bank = artist_in_bank or (lambda _name: False)
        self._image: QImage | None = None

        self._slideshow_timer = QTimer(self)
        self._slideshow_timer.timeout.connect(self._on_slideshow_tick)
        self._shuffle_queue: list[int] = []

        self.setWindowTitle(f"Post #{self.post.id} - Full Screen View")
        self.resize(1280, 850)
        self.setStyleSheet("background-color: #12141a; color: #e2e8f0;")

        # Image view
        self.image_label = QLabel("Loading image…")
        self.image_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.image_label.setMinimumSize(400, 400)
        self.image_label.setMouseTracking(True)
        self.image_label.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        self.image_label.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.image_label.customContextMenuRequested.connect(self._show_image_context_menu)
        self.image_label.setStyleSheet("background-color: #0b0d12; border: 1px solid #222630; border-radius: 4px;")
        self.image_label.installEventFilter(self)

        # Top bar
        self.top_bar_widget = QWidget()
        top_bar = QHBoxLayout(self.top_bar_widget)
        top_bar.setContentsMargins(0, 0, 0, 0)
        self.prev_btn = QPushButton("Previous (Left)")
        self.prev_btn.clicked.connect(self.prev_post)
        self.next_btn = QPushButton("Next (Right)")
        self.next_btn.clicked.connect(self.next_post)

        self.counter_label = QLabel()
        self.counter_label.setStyleSheet("font-weight: bold; font-size: 13px; color: #5b9bd5; padding: 0 10px;")

        self.tags_toggle_btn = QPushButton("Tags (T)")
        self.tags_toggle_btn.setCheckable(True)
        self.tags_toggle_btn.setChecked(True)
        self.tags_toggle_btn.clicked.connect(self._toggle_tags_panel)

        self.fs_btn = QPushButton("Fullscreen (F11)")
        self.fs_btn.clicked.connect(self._toggle_fullscreen)

        self.fav_btn = QPushButton("Favorite (L)")
        self.fav_btn.setToolTip("Favorite on e621 (L) - credits the artist")
        self.fav_btn.clicked.connect(self._on_favorite_clicked)

        self.vote_btn = QPushButton("▲ +1 (U)")
        self.vote_btn.setToolTip("Upvote on e621 (U) - increases score")
        self.vote_btn.clicked.connect(self._on_upvote_clicked)

        self.bank_btn = QPushButton("+ Spank Bank (B)")
        self.bank_btn.setToolTip("Save/remove from Spank Bank for online viewing (B)")
        self.bank_btn.clicked.connect(self._on_bank_clicked)

        self.dl_btn = QPushButton("Download (D)")
        self.dl_btn.setStyleSheet("font-weight: bold; color: #48bb78;")
        self.dl_btn.clicked.connect(lambda: self.download.emit(self.post))

        self.close_btn = QPushButton("Close (Esc)")
        self.close_btn.clicked.connect(self.close)

        top_bar.addWidget(self.prev_btn)
        top_bar.addWidget(self.next_btn)
        top_bar.addWidget(self.counter_label)
        top_bar.addStretch()
        top_bar.addWidget(self.tags_toggle_btn)
        top_bar.addWidget(self.fs_btn)
        top_bar.addWidget(self.fav_btn)
        top_bar.addWidget(self.vote_btn)
        top_bar.addWidget(self.bank_btn)
        top_bar.addWidget(self.dl_btn)
        top_bar.addWidget(self.close_btn)

        # Play / pool sub-bar
        self.play_bar_widget = QWidget()
        play_bar = QHBoxLayout(self.play_bar_widget)
        play_bar.setContentsMargins(0, 0, 0, 0)
        self.btn_slideshow = QPushButton("Play Slideshow (Space)")
        self.btn_slideshow.setCheckable(True)
        self.btn_slideshow.setStyleSheet("""
            QPushButton {
                font-weight: bold;
                padding: 4px 12px;
                background-color: #1e3a8a;
                color: #ffffff;
                border-radius: 4px;
            }
            QPushButton:checked {
                background-color: #b91c1c;
                color: #ffffff;
            }
        """)
        self.btn_slideshow.clicked.connect(self._toggle_slideshow_clicked)

        self.combo_slideshow_order = QComboBox()
        self.combo_slideshow_order.addItems(["In Order", "Shuffle"])
        self.combo_slideshow_order.setToolTip("Slideshow play order")
        self.combo_slideshow_order.currentIndexChanged.connect(self._reset_shuffle_queue)

        self.spin_slideshow_sec = QSpinBox()
        self.spin_slideshow_sec.setRange(1, 300)
        self.spin_slideshow_sec.setValue(5)
        self.spin_slideshow_sec.setSuffix(" s")
        self.spin_slideshow_sec.setToolTip("Slideshow interval in seconds")
        self.spin_slideshow_sec.valueChanged.connect(self._on_slideshow_interval_changed)

        self.cb_slideshow_loop = QCheckBox("Loop")
        self.cb_slideshow_loop.setChecked(True)
        self.cb_slideshow_loop.setToolTip("Loop back to the beginning when finished")

        self.slideshow_status = QLabel("")
        self.slideshow_status.setStyleSheet("color: #38bdf8; font-size: 12px; padding-left: 6px;")

        self.pool_view_btn = QPushButton("View Pool")
        self.pool_view_btn.setStyleSheet("font-weight: bold; color: #a78bfa;")
        self.pool_view_btn.clicked.connect(self._on_view_pool_clicked)

        self.pool_tab_btn = QPushButton("Open in New Tab")
        self.pool_tab_btn.setToolTip("Open pool in a new background tab")
        self.pool_tab_btn.setStyleSheet("font-weight: bold; color: #a78bfa;")
        self.pool_tab_btn.clicked.connect(self._on_view_pool_new_tab_clicked)

        self.pool_bank_btn = QPushButton("+ Bank")
        self.pool_bank_btn.setToolTip("Save/remove pool from Spank Bank")
        self.pool_bank_btn.setStyleSheet("font-weight: bold; color: #fbbf24;")
        self.pool_bank_btn.clicked.connect(self._on_bank_pool_clicked)

        self.pool_dl_btn = QPushButton("Download Pool")
        self.pool_dl_btn.setStyleSheet("font-weight: bold; color: #60a5fa;")
        self.pool_dl_btn.clicked.connect(self._on_download_pool_clicked)

        self.web_btn = QPushButton("Open on e621")
        self.web_btn.clicked.connect(lambda: QDesktopServices.openUrl(QUrl(self.post.page_url)))

        play_bar.addWidget(self.btn_slideshow)
        play_bar.addWidget(QLabel("Order:"))
        play_bar.addWidget(self.combo_slideshow_order)
        play_bar.addWidget(QLabel("Time:"))
        play_bar.addWidget(self.spin_slideshow_sec)
        play_bar.addWidget(self.cb_slideshow_loop)
        play_bar.addWidget(self.slideshow_status)
        play_bar.addStretch()
        play_bar.addWidget(self.pool_view_btn)
        play_bar.addWidget(self.pool_tab_btn)
        play_bar.addWidget(self.pool_bank_btn)
        play_bar.addWidget(self.pool_dl_btn)
        play_bar.addWidget(self.web_btn)

        # Right sidebar (Tags + info)
        self.right_panel = QWidget()
        rl = QVBoxLayout(self.right_panel)
        rl.setContentsMargins(4, 0, 0, 0)

        self.info_label = QLabel()
        self.info_label.setWordWrap(True)
        self.info_label.setStyleSheet("font-size: 12px; color: #cbd5e1; padding: 4px 0;")
        self.info_label.setTextFormat(Qt.TextFormat.RichText)
        self.info_label.setTextInteractionFlags(Qt.TextInteractionFlag.LinksAccessibleByMouse)
        self.info_label.linkActivated.connect(self._on_info_link)

        self.side_fav_btn = QPushButton("Favorite (e621)")
        self.side_fav_btn.setStyleSheet("font-weight: bold; padding: 5px;")
        self.side_fav_btn.clicked.connect(self._on_favorite_clicked)

        self.side_vote_btn = QPushButton("▲ +1")
        self.side_vote_btn.setStyleSheet("font-weight: bold; padding: 5px;")
        self.side_vote_btn.clicked.connect(self._on_upvote_clicked)

        side_interact_row = QHBoxLayout()
        side_interact_row.addWidget(self.side_fav_btn, 1)
        side_interact_row.addWidget(self.side_vote_btn)

        # Sidebar pool action buttons
        self.side_pool_view_btn = QPushButton("View Pool")
        self.side_pool_view_btn.setStyleSheet("font-weight: bold; color: #a78bfa; padding: 5px;")
        self.side_pool_view_btn.clicked.connect(self._on_view_pool_clicked)

        self.side_pool_tab_btn = QPushButton("+ Tab")
        self.side_pool_tab_btn.setToolTip("Open pool in a new tab")
        self.side_pool_tab_btn.setStyleSheet("font-weight: bold; color: #a78bfa; padding: 5px;")
        self.side_pool_tab_btn.clicked.connect(self._on_view_pool_new_tab_clicked)

        self.side_pool_bank_btn = QPushButton("+ Bank")
        self.side_pool_bank_btn.setToolTip("Save/remove pool from Spank Bank")
        self.side_pool_bank_btn.setStyleSheet("font-weight: bold; color: #fbbf24; padding: 5px;")
        self.side_pool_bank_btn.clicked.connect(self._on_bank_pool_clicked)

        self.side_pool_dl_btn = QPushButton("Download Pool")
        self.side_pool_dl_btn.setStyleSheet("font-weight: bold; color: #60a5fa; padding: 5px;")
        self.side_pool_dl_btn.clicked.connect(self._on_download_pool_clicked)

        side_pool_row = QHBoxLayout()
        side_pool_row.addWidget(self.side_pool_view_btn)
        side_pool_row.addWidget(self.side_pool_tab_btn)
        side_pool_row.addWidget(self.side_pool_bank_btn)
        side_pool_row.addWidget(self.side_pool_dl_btn)
        self.side_pool_widget = QWidget()
        self.side_pool_widget.setLayout(side_pool_row)
        self.side_pool_widget.setVisible(False)

        self.side_bank_btn = QPushButton("+ Spank Bank")
        self.side_bank_btn.setStyleSheet("font-weight: bold; padding: 5px;")
        self.side_bank_btn.clicked.connect(self._on_bank_clicked)

        self.side_artist_row = ArtistActionsRow(padding="5px")
        self.side_artist_row.view_requested.connect(self.artist_view)
        self.side_artist_row.view_new_tab_requested.connect(self.artist_view_new_tab)
        self.side_artist_row.bank_toggle_requested.connect(self.artist_bank_toggle)

        self.filter_edit = QLineEdit()
        self.filter_edit.setPlaceholderText("Filter tags…")
        self.filter_edit.setClearButtonEnabled(True)

        self.tag_list = TagListWidget()
        self.tag_list.artist_in_bank = lambda name: self._artist_in_bank(name)
        self.tag_list.tag_clicked.connect(self._on_tag_clicked)
        self.tag_list.tag_exclude_requested.connect(self.exclude_tag)
        self.tag_list.tag_search_requested.connect(self.search_tag)
        self.tag_list.artist_view_requested.connect(self.artist_view)
        self.tag_list.artist_view_new_tab_requested.connect(self.artist_view_new_tab)
        self.tag_list.artist_bank_toggle_requested.connect(self.artist_bank_toggle)
        self.filter_edit.textChanged.connect(self.tag_list.filter_text)

        add_btn = QPushButton("+ Add selected tag")
        add_btn.clicked.connect(self._add_selected)
        only_btn = QPushButton("Search only this tag")
        only_btn.clicked.connect(self._search_selected)

        rl.addWidget(self.info_label)
        rl.addLayout(side_interact_row)
        rl.addWidget(self.side_bank_btn)
        rl.addWidget(self.side_artist_row)
        rl.addWidget(self.side_pool_widget)
        rl.addWidget(QLabel("Tags (click to add/remove from search):"))
        rl.addWidget(self.filter_edit)
        rl.addWidget(self.tag_list, 1)
        btn_r = QHBoxLayout()
        btn_r.addWidget(add_btn)
        btn_r.addWidget(only_btn)
        rl.addLayout(btn_r)

        self.split = QSplitter()
        self.split.addWidget(self.image_label)
        self.split.addWidget(self.right_panel)
        self.split.setStretchFactor(0, 4)
        self.split.setStretchFactor(1, 1)

        self._true_fullscreen = False
        self._main_layout = QVBoxLayout(self)
        self._main_layout.setContentsMargins(10, 10, 10, 10)
        self._main_layout.addWidget(self.top_bar_widget)
        self._main_layout.addWidget(self.play_bar_widget)
        self._main_layout.addWidget(self.split, 1)

        # Prevent buttons/lists from stealing Left/Right arrow keys for focus highlight movement
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        for child in self.findChildren(QWidget):
            if isinstance(child, (QLineEdit, QSpinBox)):
                child.setFocusPolicy(Qt.FocusPolicy.ClickFocus)
            elif child is not self.image_label:
                child.setFocusPolicy(Qt.FocusPolicy.NoFocus)
            child.installEventFilter(self)
        self.installEventFilter(self)

        self.set_in_bank(in_bank)
        self.set_pool_in_bank(pool_in_bank)
        if shuffle:
            self.combo_slideshow_order.setCurrentText("Shuffle")
        self._update_display()
        self.image_label.setFocus()
        if start_slideshow:
            QTimer.singleShot(100, self.start_slideshow)

    def set_in_bank(self, in_bank: bool) -> None:
        self._in_bank = in_bank
        if in_bank:
            self.bank_btn.setText("In Bank (B)")
            self.bank_btn.setStyleSheet("font-weight: bold; background-color: #d97706; color: #ffffff; padding: 4px 10px; border-radius: 4px;")
            self.side_bank_btn.setText("In Spank Bank")
            self.side_bank_btn.setStyleSheet("font-weight: bold; background-color: #d97706; color: #ffffff; padding: 5px; border-radius: 4px;")
        else:
            self.bank_btn.setText("+ Spank Bank (B)")
            self.bank_btn.setStyleSheet("font-weight: bold; padding: 4px 10px;")
            self.side_bank_btn.setText("+ Spank Bank")
            self.side_bank_btn.setStyleSheet("font-weight: bold; padding: 5px;")

    def set_pool_in_bank(self, in_bank: bool) -> None:
        self._pool_in_bank = in_bank
        if in_bank:
            self.pool_bank_btn.setStyleSheet("font-weight: bold; background-color: #d97706; color: #ffffff; padding: 4px 10px; border-radius: 4px;")
            self.side_pool_bank_btn.setStyleSheet("font-weight: bold; background-color: #d97706; color: #ffffff; padding: 5px; border-radius: 4px;")
        else:
            self.pool_bank_btn.setStyleSheet("font-weight: bold; color: #fbbf24; padding: 4px 10px;")
            self.side_pool_bank_btn.setStyleSheet("font-weight: bold; color: #fbbf24; padding: 5px;")
        if hasattr(self, "post") and self.post:
            has_pools = bool(self.post.pools)
            if has_pools:
                if len(self.post.pools) == 1:
                    pid = self.post.pools[0]
                    txt_b = f"In Bank #{pid}" if self._pool_in_bank else f"+ Bank #{pid}"
                else:
                    txt_b = "In Bank…" if self._pool_in_bank else "+ Bank…"
                self.pool_bank_btn.setText(txt_b)
                self.side_pool_bank_btn.setText(txt_b)

    def _on_bank_clicked(self) -> None:
        self.bank_toggle.emit(self.post)

    def _on_favorite_clicked(self) -> None:
        self.favorite_toggle.emit(self.post)

    def _on_upvote_clicked(self) -> None:
        self.upvote_toggle.emit(self.post)

    def update_interactions(self) -> None:
        p = self.post
        if p.is_favorited:
            txt_fav = f"Favorited ({p.fav_count}) (L)"
            txt_side = f"Favorited ({p.fav_count})"
            style_fav = "color: #ffffff; background-color: #e11d48; font-weight: bold; padding: 4px 10px; border-radius: 4px;"
            self.fav_btn.setToolTip("Favorited on e621 (click to remove favorite)")
            self.side_fav_btn.setToolTip("Favorited on e621 (click to remove favorite)")
        else:
            txt_fav = f"Favorite ({p.fav_count}) (L)"
            txt_side = f"Favorite ({p.fav_count})"
            style_fav = "color: #fda4af; font-weight: bold; padding: 4px 10px;"
            self.fav_btn.setToolTip("Like and favorite on e621 (L) - credits the artist")
            self.side_fav_btn.setToolTip("Like and favorite on e621 - credits the artist")
        self.fav_btn.setText(txt_fav)
        self.fav_btn.setStyleSheet(style_fav)
        self.side_fav_btn.setText(txt_side)
        self.side_fav_btn.setStyleSheet(style_fav)

        if p.own_vote == 1:
            txt_vote = f"▲ Voted ({p.score}) (U)"
            txt_side_vote = f"▲ Voted ({p.score})"
            style_vote = "color: #ffffff; background-color: #16a34a; font-weight: bold; padding: 4px 10px; border-radius: 4px;"
            self.vote_btn.setToolTip("Upvoted on e621 (click to remove vote)")
            self.side_vote_btn.setToolTip("Upvoted on e621 (click to remove vote)")
        else:
            txt_vote = f"▲ +1 ({p.score}) (U)"
            txt_side_vote = f"▲ +1 ({p.score})"
            style_vote = "color: #86efac; font-weight: bold; padding: 4px 10px;"
            self.vote_btn.setToolTip("Upvote on e621 (U) - increases score")
            self.side_vote_btn.setToolTip("Upvote on e621 - increases score")
        self.vote_btn.setText(txt_vote)
        self.vote_btn.setStyleSheet(style_vote)
        self.side_vote_btn.setText(txt_side_vote)
        self.side_vote_btn.setStyleSheet(style_vote)

    def _reset_shuffle_queue(self) -> None:
        self._shuffle_queue.clear()

    def _toggle_slideshow_clicked(self, checked: bool) -> None:
        if checked:
            self.start_slideshow()
        else:
            self.pause_slideshow()

    def start_slideshow(self) -> None:
        if len(self._posts) <= 1:
            self.slideshow_status.setText("Only 1 post in view")
            return
        self.btn_slideshow.setChecked(True)
        self.btn_slideshow.setText("Pause Slideshow (Space)")
        sec = self.spin_slideshow_sec.value()
        order = self.combo_slideshow_order.currentText()
        self.slideshow_status.setText(f"Playing ({sec}s · {order})")
        self._slideshow_timer.start(sec * 1000)

    def pause_slideshow(self) -> None:
        self.btn_slideshow.setChecked(False)
        self.btn_slideshow.setText("Play Slideshow (Space)")
        self.slideshow_status.setText("Paused")
        self._slideshow_timer.stop()

    def toggle_slideshow(self) -> None:
        if self._slideshow_timer.isActive():
            self.pause_slideshow()
        else:
            self.start_slideshow()

    def _on_slideshow_interval_changed(self, val: int) -> None:
        if self._slideshow_timer.isActive():
            self._slideshow_timer.start(val * 1000)
            order = self.combo_slideshow_order.currentText()
            self.slideshow_status.setText(f"Playing ({val}s · {order})")

    def _on_slideshow_tick(self) -> None:
        order = self.combo_slideshow_order.currentText()
        if order == "Shuffle":
            if not self._shuffle_queue:
                import random
                idxs = list(range(len(self._posts)))
                if len(idxs) > 1 and self._index in idxs:
                    idxs.remove(self._index)
                random.shuffle(idxs)
                self._shuffle_queue = idxs
            if self._shuffle_queue:
                next_idx = self._shuffle_queue.pop(0)
                self._index = next_idx
                self.set_post(self._posts[self._index])
        else:
            if self._index < len(self._posts) - 1:
                self._index += 1
                self.set_post(self._posts[self._index])
            else:
                if self.cb_slideshow_loop.isChecked():
                    self._index = 0
                    self.set_post(self._posts[self._index])
                else:
                    self.pause_slideshow()
                    self.slideshow_status.setText("Finished")

    def closeEvent(self, e):  # noqa: N802
        self._slideshow_timer.stop()
        super().closeEvent(e)

    def _toggle_tags_panel(self, checked: bool) -> None:
        if self._true_fullscreen and checked:
            self._set_true_fullscreen(False)
        self.right_panel.setVisible(checked)
        QTimer.singleShot(0, self._rescale)

    def enter_fullscreen(self) -> None:
        """Enter true chromeless fullscreen mode (image fills the entire screen)."""
        self._set_true_fullscreen(True)

    def _set_true_fullscreen(self, fullscreen: bool) -> None:
        self._true_fullscreen = fullscreen
        if fullscreen:
            self.top_bar_widget.setVisible(False)
            self.play_bar_widget.setVisible(False)
            self.right_panel.setVisible(False)
            self._main_layout.setContentsMargins(0, 0, 0, 0)
            self._main_layout.setSpacing(0)
            self.image_label.setStyleSheet("background-color: #000000; border: none; border-radius: 0px;")
            self.fs_btn.setText("Controls (F11)")
            self.showFullScreen()
        else:
            self.top_bar_widget.setVisible(True)
            self.play_bar_widget.setVisible(True)
            self.right_panel.setVisible(self.tags_toggle_btn.isChecked())
            self._main_layout.setContentsMargins(10, 10, 10, 10)
            self._main_layout.setSpacing(6)
            self.image_label.setStyleSheet("background-color: #0b0d12; border: 1px solid #222630; border-radius: 4px;")
            self.fs_btn.setText("Fullscreen (F11)")
            self.showMaximized()
        self.image_label.setFocus()
        QTimer.singleShot(0, self._rescale)

    def _toggle_fullscreen(self) -> None:
        self._set_true_fullscreen(not self._true_fullscreen)

    def prev_post(self) -> None:
        if self._index > 0:
            self._index -= 1
        elif self.cb_slideshow_loop.isChecked() and len(self._posts) > 1:
            self._index = len(self._posts) - 1
        else:
            return
        self.set_post(self._posts[self._index])
        if self._slideshow_timer.isActive():
            self._slideshow_timer.start(self.spin_slideshow_sec.value() * 1000)

    def next_post(self) -> None:
        if self._index < len(self._posts) - 1:
            self._index += 1
        elif self.cb_slideshow_loop.isChecked() and len(self._posts) > 1:
            self._index = 0
        else:
            return
        self.set_post(self._posts[self._index])
        if self._slideshow_timer.isActive():
            self._slideshow_timer.start(self.spin_slideshow_sec.value() * 1000)

    def set_post(self, post: Post) -> None:
        self.post = post
        self._image = None
        self._update_display()
        self.post_changed.emit(post)

    def _update_display(self) -> None:
        post = self.post
        self.setWindowTitle(f"Post #{post.id} ({self._index + 1}/{len(self._posts)}) - Full Screen View")
        self.counter_label.setText(f"{self._index + 1} / {len(self._posts)}")
        self.prev_btn.setEnabled(self._index > 0)
        self.next_btn.setEnabled(self._index < len(self._posts) - 1)

        mb = post.size / 1_048_576 if post.size else 0
        details = [f"<b>#{post.id}</b>", post.ext.upper(), f"{post.width}×{post.height}"]
        if mb > 0:
            details.append(f"{mb:.1f} MB")
        details.append(f"Score {post.score}")
        details.append(f"Favs {post.fav_count}")
        if post.rating:
            details.append(f"Rating {post.rating.upper()}")

        pool_txt = f"<br>Pools: {', '.join(map(str, post.pools))}" if post.pools else ""
        self.info_label.setText(f"{' · '.join(details)}<br>Artist: <b>{artist_links_html(post.artists)}</b>{pool_txt}")
        self.refresh_artist_bank_state()

        has_pools = bool(post.pools)
        self.pool_view_btn.setVisible(has_pools)
        self.pool_tab_btn.setVisible(has_pools)
        self.pool_bank_btn.setVisible(has_pools)
        self.pool_dl_btn.setVisible(has_pools)
        self.side_pool_widget.setVisible(has_pools)
        if has_pools:
            if len(post.pools) == 1:
                pid = post.pools[0]
                txt_v = f"View Pool #{pid}"
                txt_d = f"Download Pool #{pid}"
                txt_b = f"In Bank #{pid}" if self._pool_in_bank else f"+ Bank #{pid}"
            else:
                txt_v = "View Pool…"
                txt_d = "Download Pool…"
                txt_b = "In Bank…" if self._pool_in_bank else "+ Bank…"
            self.pool_view_btn.setText(txt_v)
            self.pool_dl_btn.setText(txt_d)
            self.pool_bank_btn.setText(txt_b)
            self.side_pool_view_btn.setText(txt_v)
            self.side_pool_dl_btn.setText(txt_d)
            self.side_pool_bank_btn.setText(txt_b)

        self.tag_list.set_post_tags(post.tags, self._active_query)
        self.filter_edit.clear()
        self.update_interactions()

        if post.is_video or post.ext == "swf":
            self.image_label.setText(f"{post.ext.upper()} media file. Showing preview image.<br>"
                                     "Use “Open on e621” to play video.")
        else:
            self.image_label.setText("Loading high-res preview…")

    def _on_view_pool_clicked(self) -> None:
        if not self.post.pools:
            return
        if len(self.post.pools) == 1:
            self.view_pool.emit(self.post.pools[0])
        else:
            menu = QMenu(self)
            for pid in self.post.pools:
                menu.addAction(f"View Pool #{pid}", lambda pid=pid: self.view_pool.emit(pid))
            menu.exec(QCursor.pos())

    def _on_view_pool_new_tab_clicked(self) -> None:
        if not self.post.pools:
            return
        if len(self.post.pools) == 1:
            self.view_pool_new_tab.emit(self.post.pools[0])
        else:
            menu = QMenu(self)
            for pid in self.post.pools:
                menu.addAction(f"Open Pool #{pid} in new tab", lambda pid=pid: self.view_pool_new_tab.emit(pid))
            menu.exec(QCursor.pos())

    def _on_download_pool_clicked(self) -> None:
        if not self.post.pools:
            return
        if len(self.post.pools) == 1:
            self.download_pool.emit(self.post.pools[0])
        else:
            menu = QMenu(self)
            for pid in self.post.pools:
                menu.addAction(f"Download Pool #{pid}", lambda pid=pid: self.download_pool.emit(pid))
            menu.exec(QCursor.pos())

    def _on_bank_pool_clicked(self) -> None:
        if not self.post.pools:
            return
        if len(self.post.pools) == 1:
            self.bank_pool_toggle.emit(self.post.pools[0])
        else:
            menu = QMenu(self)
            for pid in self.post.pools:
                menu.addAction(f"Save/Remove Pool #{pid} in Spank Bank", lambda pid=pid: self.bank_pool_toggle.emit(pid))
            menu.exec(QCursor.pos())

    def _on_info_link(self, link: str) -> None:
        if name := artist_from_link(link):
            self.artist_view.emit(name)

    def refresh_artist_bank_state(self) -> None:
        artists = self.post.artists
        self.side_artist_row.set_artists(artists, {a for a in artists if self._artist_in_bank(a)})

    def _on_tag_clicked(self, tag: str) -> None:
        self.add_tag.emit(tag)

    def refresh_active_tags(self, active_query: str) -> None:
        self._active_query = active_query
        self.tag_list.refresh_active_tags(active_query)

    def set_image(self, image: QImage) -> None:
        self._image = image
        self._rescale()

    def set_error(self, msg: str) -> None:
        self.image_label.setText(f"Could not load image:\n{msg}")

    def resizeEvent(self, e):  # noqa: N802
        super().resizeEvent(e)
        self._rescale()

    def _rescale(self) -> None:
        if self._image is None or self._image.isNull():
            return
        pm = QPixmap.fromImage(self._image).scaled(
            self.image_label.size(), Qt.AspectRatioMode.KeepAspectRatio,
            Qt.TransformationMode.SmoothTransformation)
        self.image_label.setPixmap(pm)

    def _add_selected(self) -> None:
        if it := self.tag_list.currentItem():
            tag = it.data(Qt.ItemDataRole.UserRole) or it.text().removeprefix("[+] ").removeprefix("[-] ")
            self.add_tag.emit(tag)

    def _search_selected(self) -> None:
        if it := self.tag_list.currentItem():
            tag = it.data(Qt.ItemDataRole.UserRole) or it.text().removeprefix("[+] ").removeprefix("[-] ")
            self.search_tag.emit(tag)

    def focusNextPrevChild(self, _next: bool) -> bool:  # noqa: N802
        # Never allow arrow keys or Tab inside PreviewDialog to move focus highlight across buttons
        return False

    @staticmethod
    def _edge_zone_width(total_width: int) -> int:
        return max(120, int(total_width * 0.25))

    def click_edge_at(self, x: int, total_width: int | None = None) -> bool:
        """Navigate to previous/next post if ``x`` falls in the left or right screen edge zone."""
        w = total_width if total_width is not None else max(1, self.image_label.width())
        edge = self._edge_zone_width(w)
        if x <= edge:
            self.prev_post()
            return True
        if x >= w - edge:
            self.next_post()
            return True
        return False

    def _show_image_context_menu(self, pos) -> None:
        menu = QMenu(self)
        menu.addAction("Previous (Left)", self.prev_post)
        menu.addAction("Next (Right)", self.next_post)
        menu.addSeparator()
        slide_txt = "Pause Slideshow (Space)" if self._slideshow_timer.isActive() else "Play Slideshow (Space)"
        menu.addAction(slide_txt, self.toggle_slideshow)
        bank_txt = "Remove from Spank Bank (B)" if self._in_bank else "+ Save to Spank Bank (B)"
        menu.addAction(bank_txt, self._on_bank_clicked)
        fav_txt = "Remove Favorite (L)" if self.post.is_favorited else "Favorite on e621 (L)"
        menu.addAction(fav_txt, self._on_favorite_clicked)
        vote_txt = "Remove Upvote (U)" if self.post.own_vote == 1 else "Upvote +1 on e621 (U)"
        menu.addAction(vote_txt, self._on_upvote_clicked)
        menu.addAction("Download Post", lambda: self.download.emit(self.post))
        menu.addSeparator()
        ui_txt = "Show Controls & Tags (F11 / T)" if self._true_fullscreen else "True Fullscreen (F11)"
        menu.addAction(ui_txt, self._toggle_fullscreen)
        menu.addAction("Close (Esc)", self.close)
        menu.exec(self.image_label.mapToGlobal(pos))

    def eventFilter(self, obj, e):  # noqa: N802
        etype = e.type()
        if obj is self.image_label:
            if etype == QEvent.Type.MouseButtonPress and e.button() == Qt.MouseButton.LeftButton:
                pos = e.position().toPoint() if hasattr(e, "position") else e.pos()
                if self.click_edge_at(pos.x(), self.image_label.width()):
                    e.accept()
                    return True
            elif etype == QEvent.Type.MouseMove:
                pos = e.position().toPoint() if hasattr(e, "position") else e.pos()
                w = max(1, self.image_label.width())
                edge = self._edge_zone_width(w)
                if pos.x() <= edge or pos.x() >= w - edge:
                    self.image_label.setCursor(Qt.CursorShape.PointingHandCursor)
                else:
                    self.image_label.setCursor(Qt.CursorShape.ArrowCursor)

        if etype in (QEvent.Type.ShortcutOverride, QEvent.Type.KeyPress):
            # Let the tag filter input handle text keys when actively focused, except Escape/F11/Left/Right when empty
            if self.filter_edit.hasFocus() and e.key() not in (Qt.Key.Key_Escape, Qt.Key.Key_F11):
                if not (e.key() in (Qt.Key.Key_Left, Qt.Key.Key_Right) and not self.filter_edit.text()):
                    return super().eventFilter(obj, e)
            if self._is_handled_key(e.key()):
                if etype == QEvent.Type.KeyPress:
                    self._handle_key(e.key())
                e.accept()
                return True
        return super().eventFilter(obj, e)

    def mousePressEvent(self, e):  # noqa: N802
        if e.button() == Qt.MouseButton.LeftButton:
            pos = e.position().toPoint() if hasattr(e, "position") else e.pos()
            if self.click_edge_at(pos.x(), self.width()):
                e.accept()
                return
        super().mousePressEvent(e)

    @staticmethod
    def _is_handled_key(key: int) -> bool:
        return key in (
            Qt.Key.Key_Left, Qt.Key.Key_A,
            Qt.Key.Key_Right, Qt.Key.Key_D,
            Qt.Key.Key_Space, Qt.Key.Key_B,
            Qt.Key.Key_L, Qt.Key.Key_U,
            Qt.Key.Key_S, Qt.Key.Key_F11,
            Qt.Key.Key_F, Qt.Key.Key_Escape,
            Qt.Key.Key_Q, Qt.Key.Key_T,
        )

    def _handle_key(self, key: int) -> bool:
        if key in (Qt.Key.Key_Left, Qt.Key.Key_A):
            self.prev_post()
            return True
        if key in (Qt.Key.Key_Right, Qt.Key.Key_D):
            self.next_post()
            return True
        if key == Qt.Key.Key_Space:
            self.toggle_slideshow()
            return True
        if key == Qt.Key.Key_B:
            self._on_bank_clicked()
            return True
        if key == Qt.Key.Key_L:
            self._on_favorite_clicked()
            return True
        if key == Qt.Key.Key_U:
            self._on_upvote_clicked()
            return True
        if key == Qt.Key.Key_S:
            self.toggle_slideshow()
            return True
        if key in (Qt.Key.Key_F11, Qt.Key.Key_F):
            self._toggle_fullscreen()
            return True
        if key in (Qt.Key.Key_Escape, Qt.Key.Key_Q):
            self.close()
            return True
        if key == Qt.Key.Key_T:
            if self._true_fullscreen:
                self.tags_toggle_btn.setChecked(True)
                self._set_true_fullscreen(False)
            else:
                self.tags_toggle_btn.setChecked(not self.tags_toggle_btn.isChecked())
                self._toggle_tags_panel(self.tags_toggle_btn.isChecked())
            return True
        return False

    def keyPressEvent(self, e):  # noqa: N802
        if self._handle_key(e.key()):
            e.accept()
            return
        super().keyPressEvent(e)


# ----------------------------------------------------------------- settings
class SettingsDialog(QDialog):
    def __init__(self, settings: Settings, history_count: int, parent: QWidget | None = None):
        super().__init__(parent)
        self.setWindowTitle("Settings")
        self.resize(560, 560)
        self.clear_history_requested = False

        self.username = QLineEdit(settings.username)
        self.api_key = QLineEdit(settings.api_key)
        self.api_key.setEchoMode(QLineEdit.EchoMode.Password)
        key_help = QLabel('Create an API key at e621.net → Account → '
                          '<a href="https://e621.net/users/home">Manage API Access</a>. '
                          "Logging in unlocks posts that e621 hides from anonymous users "
                          "and lets you load private favorites.")
        key_help.setOpenExternalLinks(True)
        key_help.setWordWrap(True)

        self.blacklist = QPlainTextEdit(settings.blacklist)
        self.blacklist.setPlaceholderText("one rule per line, e.g.\ngore\nyoung -rating:s")

        self.concurrency = QSpinBox()
        self.concurrency.setRange(1, 8)
        self.concurrency.setValue(settings.concurrent_downloads)
        self.skip = QCheckBox("Skip posts already in download history (even if moved)")
        self.skip.setChecked(settings.skip_downloaded)
        self.tags_file = QCheckBox("Save a .txt file with each post's tags next to it")
        self.tags_file.setChecked(settings.write_tags_file)

        test_btn = QPushButton("Test Connection")
        test_btn.clicked.connect(lambda: self._test_connection())
        self.test_status_label = QLabel()
        self.test_status_label.setWordWrap(True)

        clear_btn = QPushButton(f"Clear download history ({history_count:,} posts)")
        clear_btn.clicked.connect(self._clear)
        self._clear_btn = clear_btn

        form = QFormLayout()
        form.addRow("e621 username:", self.username)
        form.addRow("API key:", self.api_key)
        form.addRow("", test_btn)
        form.addRow("", self.test_status_label)
        form.addRow("", key_help)
        form.addRow("Blacklist:", self.blacklist)
        form.addRow("Parallel downloads:", self.concurrency)
        form.addRow("", self.skip)
        form.addRow("", self.tags_file)
        form.addRow("", clear_btn)

        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Save
                                   | QDialogButtonBox.StandardButton.Cancel)
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)

        lay = QVBoxLayout(self)
        lay.addLayout(form)
        lay.addWidget(buttons)

    def _clear(self) -> None:
        self.clear_history_requested = True
        self._clear_btn.setText("History will be cleared on Save")
        self._clear_btn.setEnabled(False)

    def _test_connection(self) -> None:
        self.test_status_label.setText("Testing connection to e621…")
        self.test_status_label.setStyleSheet("color: #5b9bd5;")
        QApplication.processEvents()
        from .api import E621Client
        client = E621Client(self.username.text(), self.api_key.text())
        ok, msg = client.check_connection()
        if ok:
            self.test_status_label.setText(f"[OK] {msg}")
            self.test_status_label.setStyleSheet("color: #48bb78; font-weight: bold;")
        else:
            self.test_status_label.setText(f"[Failed] {msg}")
            self.test_status_label.setStyleSheet("color: #f56565; font-weight: bold;")

    def apply_to(self, s: Settings) -> None:
        s.username = self.username.text().strip()
        s.api_key = self.api_key.text().strip()
        s.blacklist = self.blacklist.toPlainText()
        s.concurrent_downloads = self.concurrency.value()
        s.skip_downloaded = self.skip.isChecked()
        s.write_tags_file = self.tags_file.isChecked()
