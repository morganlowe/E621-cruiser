"""Reusable Qt widgets."""
from __future__ import annotations

import re

from PySide6.QtCore import QRect, QSize, Qt, QTimer, QUrl, Signal
from PySide6.QtGui import (QColor, QDesktopServices, QFont, QIcon, QImage, QPainter,
                           QPixmap, QStandardItem, QStandardItemModel)
from PySide6.QtWidgets import (QAbstractItemView, QApplication, QCheckBox, QCompleter, QDialog,
                               QDialogButtonBox, QFormLayout, QHBoxLayout, QLabel,
                               QLineEdit, QListView, QListWidget, QListWidgetItem,
                               QMenu, QPlainTextEdit, QPushButton, QSpinBox, QSplitter,
                               QVBoxLayout, QWidget)

from .api import TAG_CATEGORIES, Post, Tag
from .settings import Settings

CATEGORY_COLORS = {
    0: "#5b9bd5", 1: "#e6a000", 3: "#c23bc2", 4: "#2e9e2e",
    5: "#e0601f", 6: "#d33", 7: "#888888", 8: "#2a7a2a",
}
RATING_COLORS = {"s": "#3a9e3a", "q": "#d9a400", "e": "#d23b3b"}
TILE = 150


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
        self._popup.hide()
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

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent)
        self.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.customContextMenuRequested.connect(self._show_context_menu)
        self.itemClicked.connect(self._on_item_clicked)
        self._raw_tags: dict[str, list[str]] = {}
        self._active_search_tokens: set[str] = set()

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

    def __init__(self, parent: QWidget | None = None):
        super().__init__(parent)
        self._current_post: Post | None = None

        self.title_label = QLabel("<b>No post selected</b>")
        self.title_label.setWordWrap(True)

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

        self.btn_download = QPushButton("Download")
        self.btn_download.setEnabled(False)
        self.btn_download.clicked.connect(self._emit_download)

        btn_row.addWidget(self.btn_preview)
        btn_row.addWidget(self.btn_download)

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

        layout = QVBoxLayout(self)
        layout.setContentsMargins(6, 6, 6, 6)
        layout.addWidget(self.title_label)
        layout.addWidget(self.thumb_label)
        layout.addWidget(self.info_label)
        layout.addLayout(btn_row)
        layout.addWidget(self.tag_header)
        layout.addWidget(self.filter_edit)
        layout.addWidget(self.tag_list, 1)

    def _emit_preview(self) -> None:
        if self._current_post:
            self.preview_requested.emit(self._current_post)

    def _emit_download(self) -> None:
        if self._current_post:
            self.download_requested.emit(self._current_post)

    def _on_filter_changed(self, text: str) -> None:
        self.tag_list.filter_text(text)

    def set_post(self, post: Post | None, thumb_image: QImage | None = None, active_query: str = "") -> None:
        self._current_post = post
        self.filter_edit.clear()
        if post is None:
            self.title_label.setText("<b>No post selected</b>")
            self.thumb_label.clear()
            self.thumb_label.setText("Select a post")
            self.info_label.setText("Click an image in the grid to view tags.")
            self.btn_preview.setEnabled(False)
            self.btn_download.setEnabled(False)
            self.tag_list.clear()
            return

        artists = ", ".join(post.artists) or "unknown"
        self.title_label.setText(f"<b>Post #{post.id}</b> · {artists}")
        self.btn_preview.setEnabled(True)
        self.btn_download.setEnabled(True)

        mb = post.size / 1_048_576 if post.size else 0
        details = [f"{post.ext.upper()}", f"{post.width}×{post.height}"]
        if mb > 0:
            details.append(f"{mb:.1f} MB")
        details.append(f"Score {post.score}")
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
    post_changed = Signal(object)

    def __init__(self, post: Post, all_posts: list[Post] | None = None, index: int = 0,
                 active_query: str = "", parent: QWidget | None = None):
        super().__init__(parent)
        self._posts = all_posts if all_posts else [post]
        self._index = max(0, min(len(self._posts) - 1, index))
        self.post = self._posts[self._index]
        self._active_query = active_query
        self._image: QImage | None = None

        self.setWindowTitle(f"Post #{self.post.id} - Full Screen View")
        self.resize(1280, 850)
        self.setStyleSheet("background-color: #12141a; color: #e2e8f0;")

        # Image view
        self.image_label = QLabel("Loading image…")
        self.image_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.image_label.setMinimumSize(400, 400)
        self.image_label.setStyleSheet("background-color: #0b0d12; border: 1px solid #222630; border-radius: 4px;")

        # Top bar
        top_bar = QHBoxLayout()
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

        self.dl_btn = QPushButton("Download (D)")
        self.dl_btn.setStyleSheet("font-weight: bold; color: #48bb78;")
        self.dl_btn.clicked.connect(lambda: self.download.emit(self.post))

        self.web_btn = QPushButton("Open on e621")
        self.web_btn.clicked.connect(lambda: QDesktopServices.openUrl(QUrl(self.post.page_url)))

        self.close_btn = QPushButton("Close (Esc)")
        self.close_btn.clicked.connect(self.close)

        top_bar.addWidget(self.prev_btn)
        top_bar.addWidget(self.next_btn)
        top_bar.addWidget(self.counter_label)
        top_bar.addStretch()
        top_bar.addWidget(self.tags_toggle_btn)
        top_bar.addWidget(self.fs_btn)
        top_bar.addWidget(self.dl_btn)
        top_bar.addWidget(self.web_btn)
        top_bar.addWidget(self.close_btn)

        # Right sidebar (Tags + info)
        self.right_panel = QWidget()
        rl = QVBoxLayout(self.right_panel)
        rl.setContentsMargins(4, 0, 0, 0)

        self.info_label = QLabel()
        self.info_label.setWordWrap(True)
        self.info_label.setStyleSheet("font-size: 12px; color: #cbd5e1; padding: 4px 0;")

        self.filter_edit = QLineEdit()
        self.filter_edit.setPlaceholderText("Filter tags…")
        self.filter_edit.setClearButtonEnabled(True)

        self.tag_list = TagListWidget()
        self.tag_list.tag_clicked.connect(self._on_tag_clicked)
        self.tag_list.tag_exclude_requested.connect(self.exclude_tag)
        self.tag_list.tag_search_requested.connect(self.search_tag)
        self.filter_edit.textChanged.connect(self.tag_list.filter_text)

        add_btn = QPushButton("+ Add selected tag")
        add_btn.clicked.connect(self._add_selected)
        only_btn = QPushButton("Search only this tag")
        only_btn.clicked.connect(self._search_selected)

        rl.addWidget(self.info_label)
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

        lay = QVBoxLayout(self)
        lay.setContentsMargins(10, 10, 10, 10)
        lay.addLayout(top_bar)
        lay.addWidget(self.split, 1)

        self._update_display()

    def _toggle_tags_panel(self, checked: bool) -> None:
        self.right_panel.setVisible(checked)

    def _toggle_fullscreen(self) -> None:
        if self.isFullScreen():
            self.showMaximized()
        else:
            self.showFullScreen()

    def prev_post(self) -> None:
        if self._index > 0:
            self._index -= 1
            self.set_post(self._posts[self._index])

    def next_post(self) -> None:
        if self._index < len(self._posts) - 1:
            self._index += 1
            self.set_post(self._posts[self._index])

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

        artists = ", ".join(post.artists) or "unknown"
        mb = post.size / 1_048_576 if post.size else 0
        details = [f"<b>#{post.id}</b>", post.ext.upper(), f"{post.width}×{post.height}"]
        if mb > 0:
            details.append(f"{mb:.1f} MB")
        details.append(f"Score {post.score}")
        details.append(f"Favs {post.fav_count}")
        if post.rating:
            details.append(f"Rating {post.rating.upper()}")

        pool_txt = f"<br>Pools: {', '.join(map(str, post.pools))}" if post.pools else ""
        self.info_label.setText(f"{' · '.join(details)}<br>Artist: <b>{artists}</b>{pool_txt}")

        self.tag_list.set_post_tags(post.tags, self._active_query)
        self.filter_edit.clear()

        if post.is_video or post.ext == "swf":
            self.image_label.setText(f"{post.ext.upper()} media file. Showing preview image.<br>"
                                     "Use “Open on e621” to play video.")
        else:
            self.image_label.setText("Loading high-res preview…")

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

    def keyPressEvent(self, e):  # noqa: N802
        if e.key() in (Qt.Key.Key_Left, Qt.Key.Key_A):
            self.prev_post()
            return
        if e.key() in (Qt.Key.Key_Right, Qt.Key.Key_D):
            self.next_post()
            return
        if e.key() == Qt.Key.Key_Escape or e.key() == Qt.Key.Key_Q:
            self.close()
            return
        if e.key() == Qt.Key.Key_F11:
            self._toggle_fullscreen()
            return
        if e.key() == Qt.Key.Key_T:
            self.tags_toggle_btn.setChecked(not self.tags_toggle_btn.isChecked())
            self._toggle_tags_panel(self.tags_toggle_btn.isChecked())
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
