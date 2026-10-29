import unittest
from pathlib import Path

from e621cruiser.api import Pool, Post
from e621cruiser.blacklist import Blacklist
from e621cruiser.storage import DownloadJob, build_target, sanitize


def make_post(**kw) -> Post:
    d = {
        "id": 100,
        "file": {"md5": "abcdef", "ext": "png", "url": "https://x/abcdef.png",
                 "width": 10, "height": 10, "size": 1},
        "preview": {"url": "https://x/p.jpg"},
        "sample": {"has": False},
        "score": {"total": 5},
        "rating": "e",
        "tags": {"artist": ["conditional_dnp", "someartist"], "general": ["solo", "smile"],
                 "species": ["wolf"], "copyright": [], "character": []},
        "pools": [],
    }
    d.update(kw)
    return Post.from_json(d)


class BlacklistTests(unittest.TestCase):
    def test_rules(self):
        p = make_post()
        self.assertTrue(Blacklist("wolf").is_blocked(p))
        self.assertFalse(Blacklist("fox").is_blocked(p))
        self.assertTrue(Blacklist("wolf solo").is_blocked(p))
        self.assertFalse(Blacklist("wolf -rating:e").is_blocked(p))
        self.assertTrue(Blacklist("solo rating:e").is_blocked(p))
        self.assertTrue(Blacklist("~fox ~wolf").is_blocked(p))
        self.assertFalse(Blacklist("# wolf\n-solo").is_blocked(p))
        self.assertTrue(Blacklist("type:png").is_blocked(p))


class PathTests(unittest.TestCase):
    def job(self, mode, **kw):
        return DownloadJob(post=kw.pop("post", make_post()), base_dir="/dl", mode=mode, **kw)

    def test_modes(self):
        none = lambda pid: None  # noqa: E731
        self.assertEqual(build_target(self.job("artist"), none), Path("/dl/artists/someartist/100.png"))
        self.assertEqual(build_target(self.job("species"), none), Path("/dl/species/wolf/100.png"))
        self.assertEqual(build_target(self.job("copyright"), none),
                         Path("/dl/copyrights/_unknown_copyright/100.png"))
        self.assertEqual(build_target(self.job("search", source="wolf solo"), none),
                         Path("/dl/searches/wolf solo/100.png"))
        self.assertEqual(build_target(self.job("flat"), none), Path("/dl/100.png"))
        self.assertEqual(build_target(self.job("rating"), none), Path("/dl/ratings/explicit/100.png"))
        self.assertEqual(build_target(self.job("search", pool_id=7, pool_name="My Comic", pool_index=3), none),
                         Path("/dl/pools/My Comic (7)/0003_100.png"))

    def test_pool_lookup(self):
        post = make_post(pools=[9])
        pool = Pool(9, "Some_Comic", [50, 100])
        self.assertEqual(build_target(self.job("pool", post=post), lambda pid: pool),
                         Path("/dl/pools/Some Comic (9)/0002_100.png"))

    def test_sanitize(self):
        self.assertEqual(sanitize('a/b:c*?"<>|'), "a_b_c______")
        self.assertEqual(sanitize("..."), "_")
        self.assertEqual(sanitize("con"), "_con_")
        self.assertEqual(sanitize("AUX"), "_AUX_")


class PostTests(unittest.TestCase):
    def test_artists_filtered(self):
        self.assertEqual(make_post().artists, ["someartist"])

    def test_missing_url(self):
        p = make_post(file={"md5": "x", "ext": "jpg", "url": None})
        self.assertIsNone(p.file_url)


class WidgetQueryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import os
        from PySide6.QtWidgets import QApplication
        os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
        os.environ["XDG_DATA_HOME"] = "/tmp/e621_test_data"
        os.environ["XDG_CONFIG_HOME"] = "/tmp/e621_test_config"
        cls.app = QApplication.instance() or QApplication([])

    def test_insert_tag(self):
        from e621cruiser.widgets import TagLineEdit
        edit = TagLineEdit()
        edit.setText("solo can")
        edit.setCursorPosition(len("solo can"))
        edit._insert_tag("canine", add_space=False)
        self.assertEqual(edit.text(), "solo canine")

        edit.setText("solo can")
        edit.setCursorPosition(len("solo can"))
        edit._insert_tag("canine", add_space=True)
        self.assertEqual(edit.text(), "solo canine ")

    def test_autocomplete_local_match_and_tab(self):
        from PySide6.QtCore import Qt, QEvent
        from PySide6.QtGui import QKeyEvent
        from e621cruiser.widgets import TagLineEdit
        edit = TagLineEdit()
        edit.show()
        edit.setText("wo")
        edit.setCursorPosition(2)
        edit._on_text_edited("wo")
        self.assertTrue(edit._popup.isVisible())
        self.assertGreaterEqual(edit._popup.count(), 1)
        # Tab complete
        key_tab = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_Tab, Qt.KeyboardModifier.NoModifier)
        edit.keyPressEvent(key_tab)
        self.assertEqual(edit.text(), "wolf ")
        self.assertFalse(edit._popup.isVisible())
        edit.close()

    def test_empty_startup_state(self):
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings
        w = MainWindow(Settings())
        w.api.submit = lambda *args, **kwargs: None
        self.assertEqual(w.grid.count(), 0)
        self.assertIn("Search for tags", w.result_label.text())
        w.close()

    def test_search_with_clicked_bool(self):
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings
        w = MainWindow(Settings())
        w.api.submit = lambda *args, **kwargs: None
        w.tag_edit.setText("wolf")
        # Simulate button clicked passing False as raw argument
        w._start_search(False)
        self.assertEqual(w._source.query, "wolf")
        w.close()

    def test_compose_query_filters(self):
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings
        w = MainWindow(Settings())
        w.api.submit = lambda *args, **kwargs: None
        w.tag_edit.setText("wolf")

        # Set media filter to Drawings only (-animated)
        idx = w.media_filter.findData("-animated")
        w.media_filter.setCurrentIndex(idx)
        # Check pool only
        w.cb_in_pool.setChecked(True)
        # Check quick blacklist items: gore and scat
        if "gore" in w.quick_bl_checkboxes:
            w.quick_bl_checkboxes["gore"].setChecked(True)
        if "scat" in w.quick_bl_checkboxes:
            w.quick_bl_checkboxes["scat"].setChecked(True)

        composed = w._compose_query("wolf")
        self.assertIn("wolf", composed)
        self.assertIn("-animated", composed)
        self.assertIn("pool:any", composed)
        self.assertIn("-gore", composed)
        self.assertIn("-scat", composed)

        # Ensure no duplicate if tag already contains pool: or -animated
        composed2 = w._compose_query("wolf pool:123 -animated")
        self.assertNotIn("pool:any", composed2)
        self.assertEqual(composed2.count("-animated"), 1)

        w.close()

    def test_full_blacklist_and_settings(self):
        from e621cruiser.settings import Settings, full_blacklist
        s = Settings(
            blacklist="feral\ncomic",
            quick_blacklist=["gore", "young"]
        )
        fb = full_blacklist(s)
        self.assertIn("feral", fb)
        self.assertIn("comic", fb)
        self.assertIn("gore", fb)
        self.assertIn("young", fb)

        # Test dictionary serialization round-trip
        data = s.to_dict()
        s2 = Settings.from_dict(data)
        self.assertEqual(s2.quick_blacklist, ["gore", "young"])

    def test_collapsible_section(self):
        from PySide6.QtWidgets import QLabel
        from e621cruiser.widgets import CollapsibleSection
        sec = CollapsibleSection("Test Section", expanded=False)
        lbl = QLabel("Inner text")
        sec.add_widget(lbl)
        self.assertFalse(sec.is_expanded())
        self.assertTrue(sec._body.isHidden())
        self.assertIn("[+] Test Section", sec._header.text())

        # Toggle expanded
        sec.set_expanded(True)
        self.assertTrue(sec.is_expanded())
        self.assertFalse(sec._body.isHidden())
        self.assertIn("[-] Test Section", sec._header.text())

        # Test badge
        sec.set_badge("3 active")
        self.assertIn("[3 active]", sec._header.text())

    def test_main_window_collapsible_and_width(self):
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings
        s = Settings(expand_blacklist=False, expand_saved=False)
        w = MainWindow(s)
        # Verify left panel minimum width is at least 350 to prevent clipping
        self.assertGreaterEqual(w.tabs.minimumWidth(), 350)
        self.assertFalse(w.bl_section.is_expanded())
        self.assertFalse(w.saved_section.is_expanded())

        # Toggle blacklist
        w.bl_section.set_expanded(True)
        self.assertTrue(w.bl_section.is_expanded())
        w.close()

    def test_pool_buttons_in_preview_and_inspector(self):
        from e621cruiser.widgets import PreviewDialog, PostInspector
        p_with_pool = make_post(pools=[30725])
        p_no_pool = make_post(pools=[])

        # Test PostInspector
        inspector = PostInspector()
        inspector.set_post(p_with_pool)
        self.assertFalse(inspector.pool_widget.isHidden())
        self.assertIn("30725", inspector.btn_view_pool.text())
        self.assertIn("30725", inspector.btn_download_pool.text())

        pool_view_signals = []
        pool_dl_signals = []
        inspector.view_pool_requested.connect(pool_view_signals.append)
        inspector.download_pool_requested.connect(pool_dl_signals.append)

        inspector.btn_view_pool.click()
        inspector.btn_download_pool.click()
        self.assertEqual(pool_view_signals, [30725])
        self.assertEqual(pool_dl_signals, [30725])

        inspector.set_post(p_no_pool)
        self.assertTrue(inspector.pool_widget.isHidden())

        # Test PreviewDialog
        dlg = PreviewDialog(p_with_pool)
        self.assertFalse(dlg.pool_view_btn.isHidden())
        self.assertFalse(dlg.pool_dl_btn.isHidden())
        self.assertFalse(dlg.side_pool_widget.isHidden())
        self.assertIn("30725", dlg.pool_view_btn.text())
        self.assertIn("30725", dlg.pool_dl_btn.text())

        dlg_view_signals = []
        dlg_dl_signals = []
        dlg.view_pool.connect(dlg_view_signals.append)
        dlg.download_pool.connect(dlg_dl_signals.append)

        dlg.pool_view_btn.click()
        dlg.pool_dl_btn.click()
        self.assertEqual(dlg_view_signals, [30725])
        self.assertEqual(dlg_dl_signals, [30725])

        # Change post to no pool
        dlg.set_post(p_no_pool)
        self.assertTrue(dlg.pool_view_btn.isHidden())
        self.assertTrue(dlg.pool_dl_btn.isHidden())
        self.assertTrue(dlg.side_pool_widget.isHidden())
        dlg.close()
        dlg.deleteLater()
        inspector.deleteLater()

    def test_tabs_and_navigation(self):
        from e621cruiser.api import Pool
        from e621cruiser.main_window import MainWindow, ResultTab, Source
        from e621cruiser.settings import Settings

        w = MainWindow(Settings())
        w.api.submit = lambda *args, **kwargs: None
        self.assertEqual(w.results_tabs.count(), 1)
        initial_tab = w.current_tab
        self.assertIsInstance(initial_tab, ResultTab)
        self.assertFalse(initial_tab.btn_back.isEnabled())
        self.assertFalse(initial_tab.btn_forward.isEnabled())
        self.assertFalse(initial_tab.btn_dl_pool.isVisible())

        # Create new tab
        tab2 = w._new_empty_tab()
        self.assertEqual(w.results_tabs.count(), 2)
        self.assertEqual(w.current_tab, tab2)

        # Close current tab
        w._close_current_tab()
        self.assertEqual(w.results_tabs.count(), 1)
        self.assertEqual(w.current_tab, initial_tab)

        # Closing the last tab should not delete the tab widget but reset
        w._close_current_tab()
        self.assertEqual(w.results_tabs.count(), 1)

        # Add posts and verify back/forward navigation
        p1 = make_post(id=101)
        p2 = make_post(id=102)
        w._show_posts([p1, p2])
        self.assertEqual(w.grid.count(), 2)

        # Navigate to a pool in same tab
        test_pool = Pool(id=999, name="Test_Comic", description="", post_ids=[101, 102], category="series")
        w._open_pool(test_pool, new_tab=False)
        self.assertEqual(w._source.kind, "pool")
        self.assertFalse(w.current_tab.btn_dl_pool.isHidden())
        self.assertIn("Download Pool", w.current_tab.btn_dl_pool.text())
        self.assertIn("Test Comic", w.result_label.text())
        self.assertTrue(w.current_tab.btn_back.isEnabled())
        self.assertFalse(w.current_tab.btn_forward.isEnabled())

        # Test back navigation
        w._navigate_back()
        self.assertEqual(w._source.kind, "search")
        self.assertEqual(w.grid.count(), 2)
        self.assertTrue(w.current_tab.btn_dl_pool.isHidden())
        self.assertTrue(w.current_tab.btn_forward.isEnabled())

        # Test forward navigation
        w._navigate_forward()
        self.assertEqual(w._source.kind, "pool")
        self.assertFalse(w.current_tab.btn_dl_pool.isHidden())

        # Open pool in a new tab
        w._open_pool(test_pool, new_tab=True)
        self.assertEqual(w.results_tabs.count(), 2)
        self.assertEqual(w.current_tab.source.kind, "pool")

        w.close()

    def test_spank_bank_storage(self):
        import tempfile
        from pathlib import Path
        from e621cruiser.storage import SpankBank

        with tempfile.TemporaryDirectory() as tmpdir:
            bank_db = Path(tmpdir) / "spank_bank.db"
            bank = SpankBank(bank_db)
            self.assertEqual(len(bank), 0)

            p1 = make_post(id=201)
            p2 = make_post(id=202)

            self.assertFalse(bank.has(201))
            bank.add(p1)
            self.assertTrue(bank.has(201))
            self.assertEqual(len(bank), 1)

            # Toggle behavior
            added = bank.toggle(p2)
            self.assertTrue(added)
            self.assertTrue(bank.has(202))
            self.assertEqual(len(bank), 2)

            removed = bank.toggle(p2)
            self.assertFalse(removed)
            self.assertFalse(bank.has(202))
            self.assertEqual(len(bank), 1)

            # Retrieve all posts
            all_posts = bank.get_all()
            self.assertEqual(len(all_posts), 1)
            self.assertEqual(all_posts[0].id, 201)

            # Clear
            bank.clear()
            self.assertEqual(len(bank), 0)
            self.assertFalse(bank.has(201))

    def test_spank_bank_and_slideshow_ui(self):
        from PySide6.QtCore import QEvent, Qt
        from PySide6.QtGui import QKeyEvent
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings
        from e621cruiser.widgets import PreviewDialog

        w = MainWindow(Settings())
        w.api.submit = lambda *args, **kwargs: None
        w.spank_bank.clear()

        p1 = make_post(id=301)
        p2 = make_post(id=302)
        p3 = make_post(id=303)

        # Toggle Spank Bank from MainWindow
        self.assertFalse(w.spank_bank.has(301))
        w._toggle_spank_bank(p1)
        self.assertTrue(w.spank_bank.has(301))
        self.assertIn("1 saved post", w.bank_summary_label.text())

        # Inspector updates
        w.inspector.set_post(p1, in_bank=w.spank_bank.has(301))
        self.assertEqual(w.inspector.btn_bank.text(), "In Bank")

        # Open Spank Bank in a result tab
        w._open_spank_bank(new_tab=True)
        self.assertEqual(w.current_tab.source.kind, "spank_bank")
        self.assertEqual(w.grid.count(), 1)
        self.assertIn("Spank Bank", w.result_label.text())

        # Test PreviewDialog slideshow controls
        dlg = PreviewDialog(p1, all_posts=[p1, p2, p3], in_bank=True)
        self.assertEqual(dlg.bank_btn.text(), "In Bank (B)")
        self.assertFalse(dlg._slideshow_timer.isActive())

        # Start slideshow
        dlg.start_slideshow()
        self.assertTrue(dlg._slideshow_timer.isActive())
        self.assertIn("Pause Slideshow", dlg.btn_slideshow.text())

        # Tick sequential
        dlg._on_slideshow_tick()
        self.assertEqual(dlg.post.id, 302)
        dlg._on_slideshow_tick()
        self.assertEqual(dlg.post.id, 303)
        # Loop back to 0
        dlg._on_slideshow_tick()
        self.assertEqual(dlg.post.id, 301)

        # Test Shuffle order
        dlg.combo_slideshow_order.setCurrentText("Shuffle")
        dlg._on_slideshow_tick()
        self.assertIn(dlg.post.id, [301, 302, 303])

        # Test Space key pauses/resumes
        key_space = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_Space, Qt.KeyboardModifier.NoModifier)
        dlg.keyPressEvent(key_space)
        self.assertFalse(dlg._slideshow_timer.isActive())
        dlg.keyPressEvent(key_space)
        self.assertTrue(dlg._slideshow_timer.isActive())

        dlg.close()
        dlg.deleteLater()
        w.close()

    def test_spank_bank_pools_storage(self):
        import tempfile
        from e621cruiser.storage import SpankBank
        with tempfile.NamedTemporaryFile(suffix=".db") as f:
            bank = SpankBank(Path(f.name))
            self.assertEqual(bank.pool_count, 0)
            self.assertFalse(bank.has_pool(55))

            pool1 = Pool(55, "Test_Comic_1", [101, 102], category="series", description="A fun comic")
            pool2 = Pool(56, "Test_Comic_2", [201, 202, 203], category="collection")

            # Add pool
            bank.add_pool(pool1)
            self.assertTrue(bank.has_pool(55))
            self.assertEqual(bank.pool_count, 1)

            # Get all pools
            all_pools = bank.get_all_pools()
            self.assertEqual(len(all_pools), 1)
            self.assertEqual(all_pools[0].id, 55)
            self.assertEqual(all_pools[0].name, "Test_Comic_1")
            self.assertEqual(all_pools[0].post_count, 2)
            self.assertEqual(all_pools[0].category, "series")

            # Toggle pool
            self.assertTrue(bank.toggle_pool(pool2))
            self.assertEqual(bank.pool_count, 2)
            self.assertFalse(bank.toggle_pool(pool2))
            self.assertEqual(bank.pool_count, 1)

            # Remove pool
            bank.remove_pool(55)
            self.assertEqual(bank.pool_count, 0)
            self.assertFalse(bank.has_pool(55))

            # Clear pools
            bank.add_pool(pool1)
            bank.add_pool(pool2)
            self.assertEqual(bank.pool_count, 2)
            bank.clear_pools()
            self.assertEqual(bank.pool_count, 0)

    def test_blacklist_persistence(self):
        import tempfile
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings

        with tempfile.TemporaryDirectory() as tmpdir:
            cfg_path = Path(tmpdir) / "settings.json"
            s = Settings.load(cfg_path)
            s.save()

            w = MainWindow(s)
            w.api.submit = lambda *args, **kwargs: None

            # Toggle a checkbox directly and verify instant save on disk
            self.assertIn("gore", w.quick_bl_checkboxes)
            w.quick_bl_checkboxes["gore"].setChecked(True)
            self.assertIn("gore", w.settings.quick_blacklist)

            # Verify saved to disk file
            reloaded = Settings.load(cfg_path)
            self.assertIn("gore", reloaded.quick_blacklist)

            # Click Select Common
            w._select_common_blacklist()
            reloaded = Settings.load(cfg_path)
            self.assertIn("scat", reloaded.quick_blacklist)
            self.assertIn("young", reloaded.quick_blacklist)

            # Click Clear All
            w._clear_quick_blacklist()
            reloaded = Settings.load(cfg_path)
            self.assertEqual(reloaded.quick_blacklist, [])

            # Toggle section collapse
            w._on_blacklist_section_toggled(False)
            reloaded = Settings.load(cfg_path)
            self.assertFalse(reloaded.expand_blacklist)

            w.close()

    def test_main_window_spank_bank_pools_ui(self):
        import tempfile
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings

        with tempfile.TemporaryDirectory() as tmpdir:
            cfg_path = Path(tmpdir) / "settings.json"
            s = Settings.load(cfg_path)
            w = MainWindow(s)
            w.api.submit = lambda *args, **kwargs: None
            w.spank_bank.clear()
            w.spank_bank.clear_pools()

            pool = Pool(77, "Adventures_of_Fox", [701, 702, 703], category="series")

            # Toggle pool into bank
            added = w._toggle_pool_in_bank(pool)
            self.assertTrue(added)
            self.assertTrue(w.spank_bank.has_pool(77))

            # UI labels and subtabs
            self.assertIn("1 saved pool", w.bank_pools_summary_label.text())
            self.assertEqual(w.bank_subtabs.tabText(0), "Pools (1)")
            self.assertEqual(w.bank_subtabs.tabText(1), "Posts (0)")
            self.assertEqual(w.bank_pools_list.count(), 1)

            # Check selected pool
            w.bank_pools_list.setCurrentRow(0)
            sel_pool = w._selected_bank_saved_pool()
            self.assertIsNotNone(sel_pool)
            self.assertEqual(sel_pool.id, 77)

            # Filter pool list
            w.bank_pools_filter_edit.setText("Fox")
            self.assertEqual(w.bank_pools_list.count(), 1)
            w.bank_pools_filter_edit.setText("NonExistent")
            self.assertEqual(w.bank_pools_list.count(), 0)
            w.bank_pools_filter_edit.clear()
            self.assertEqual(w.bank_pools_list.count(), 1)

            # Test pool button in current tab
            w._open_pool(pool)
            self.assertEqual(w.current_tab.source.kind, "pool")
            self.assertEqual(w.current_tab.btn_bank_pool.text(), "In Bank")

            # Toggle out of bank via tab button
            w._toggle_current_pool_in_bank()
            self.assertFalse(w.spank_bank.has_pool(77))
            self.assertEqual(w.current_tab.btn_bank_pool.text(), "+ Bank")

            w.close()

    def test_e621_interactions(self):
        from unittest.mock import MagicMock
        from PySide6.QtGui import QKeyEvent
        from PySide6.QtCore import QEvent, Qt
        from e621cruiser.api import E621Client, Post
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings
        from e621cruiser.widgets import PostInspector, PreviewDialog

        # Test Post dataclass and from_json with is_favorited and own_vote
        p_data = {
            "id": 999,
            "file": {"md5": "123", "ext": "jpg", "url": "https://x/123.jpg", "width": 100, "height": 100, "size": 500},
            "preview": {"url": "https://x/p.jpg"},
            "sample": {"has": False},
            "score": {"total": 42},
            "fav_count": 15,
            "is_favorited": True,
            "own_vote": 1,
            "tags": {"artist": ["superartist"]},
        }
        post = Post.from_json(p_data)
        self.assertEqual(post.id, 999)
        self.assertEqual(post.score, 42)
        self.assertEqual(post.fav_count, 15)
        self.assertTrue(post.is_favorited)
        self.assertEqual(post.own_vote, 1)

        # Test Client interaction methods with mocks
        client = E621Client("testuser", "testkey")
        client._post = MagicMock(return_value={"is_favorited": True})
        client._delete = MagicMock(return_value={})

        client.add_favorite(999)
        client._post.assert_called_with("/favorites.json", data={"post_id": "999"})

        client.remove_favorite(999)
        client._delete.assert_called_with("/favorites/999.json")

        client.vote_post(999, 1)
        client._post.assert_called_with("/posts/999/votes.json", data={"score": "1"})

        client.remove_vote(999)
        client._delete.assert_called_with("/posts/999/votes.json")

        # Test PostInspector Favorite and Upvote UI
        inspector = PostInspector()
        inspector.set_post(post)
        self.assertIn("Favorited (15)", inspector.btn_fav.text())
        self.assertIn("▲ Voted (42)", inspector.btn_upvote.text())

        # Test PostInspector signals
        fav_signal_emitted = []
        vote_signal_emitted = []
        inspector.favorite_toggle_requested.connect(lambda p: fav_signal_emitted.append(p.id))
        inspector.upvote_toggle_requested.connect(lambda p: vote_signal_emitted.append(p.id))

        inspector.btn_fav.click()
        self.assertEqual(fav_signal_emitted, [999])
        inspector.btn_upvote.click()
        self.assertEqual(vote_signal_emitted, [999])
        inspector.deleteLater()

        # Test PreviewDialog interactions
        dlg = PreviewDialog(post, all_posts=[post], in_bank=False)
        self.assertIn("Favorited (15)", dlg.fav_btn.text())
        self.assertIn("▲ Voted (42)", dlg.vote_btn.text())

        dlg_fav_emitted = []
        dlg_vote_emitted = []
        dlg.favorite_toggle.connect(lambda p: dlg_fav_emitted.append(p.id))
        dlg.upvote_toggle.connect(lambda p: dlg_vote_emitted.append(p.id))

        # Test L shortcut for favorite
        key_l = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_L, Qt.KeyboardModifier.NoModifier)
        dlg.keyPressEvent(key_l)
        self.assertEqual(dlg_fav_emitted, [999])

        # Test U shortcut for upvote
        key_u = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_U, Qt.KeyboardModifier.NoModifier)
        dlg.keyPressEvent(key_u)
        self.assertEqual(dlg_vote_emitted, [999])

        dlg.close()
        dlg.deleteLater()

        # Test MainWindow _toggle_favorite and _toggle_upvote
        s = Settings(username="testuser", api_key="secretkey")
        w = MainWindow(s)
        # Mock API submit to run synchronously
        w.api.submit = lambda func, *args, **kwargs: kwargs["on_result"](func())
        w.client = client

        # Toggle favorite off (was True)
        w._toggle_favorite(post)
        self.assertFalse(post.is_favorited)
        self.assertEqual(post.fav_count, 14)

        # Toggle favorite back on (was False)
        w._toggle_favorite(post)
        self.assertTrue(post.is_favorited)
        self.assertEqual(post.fav_count, 15)

        # Toggle upvote off (was 1)
        w._toggle_upvote(post)
        self.assertEqual(post.own_vote, 0)
        self.assertEqual(post.score, 41)

        # Toggle upvote back on
        w._toggle_upvote(post)
        self.assertEqual(post.own_vote, 1)
        self.assertEqual(post.score, 42)

        w.close()

    def test_spank_bank_artists_storage(self):
        import tempfile
        from e621cruiser.storage import SpankBank

        with tempfile.TemporaryDirectory() as tmpdir:
            bank_db = Path(tmpdir) / "spank_bank.db"
            bank = SpankBank(bank_db)

            self.assertEqual(bank.artist_count, 0)
            self.assertFalse(bank.has_artist("SomeArtist"))

            # Add artist (normalizes case & spaces)
            self.assertTrue(bank.toggle_artist("  Some Artist  "))
            self.assertTrue(bank.has_artist("some_artist"))
            self.assertTrue(bank.has_artist("SOME ARTIST"))
            self.assertEqual(bank.artist_count, 1)

            bank.add_artist("another_artist")
            self.assertEqual(bank.artist_count, 2)
            self.assertEqual(bank.get_all_artists(), ["another_artist", "some_artist"])

            # Persistence across reopen
            bank2 = SpankBank(bank_db)
            self.assertEqual(bank2.artist_count, 2)
            self.assertTrue(bank2.has_artist("some_artist"))

            # Toggle remove
            self.assertFalse(bank2.toggle_artist("some_artist"))
            self.assertFalse(bank2.has_artist("some_artist"))
            self.assertEqual(bank2.artist_count, 1)

            bank2.clear_artists()
            self.assertEqual(bank2.artist_count, 0)

    def test_artist_links_and_spank_bank_ui(self):
        import tempfile
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings
        from e621cruiser.widgets import PostInspector, PreviewDialog, artist_from_link, artist_links_html

        # Test link encoding/decoding with special characters
        html_str = artist_links_html(["cool_fox", "artist_(2)"])
        self.assertIn("artist:cool_fox", html_str)
        self.assertEqual(artist_from_link("artist:artist_%282%29"), "artist_(2)")

        post = make_post(id=555, tags={"artist": ["conditional_dnp", "fox_painter"],
                                       "general": ["solo"], "species": ["fox"],
                                       "copyright": [], "character": []})

        # Inspector clickable artist link & buttons
        insp = PostInspector()
        insp.set_post(post, artists_banked=set())
        self.assertFalse(insp.artist_row.isHidden())
        self.assertEqual(insp.artist_row.btn_bank.text(), "+ Bank Artist")

        viewed = []
        banked = []
        insp.artist_view_requested.connect(viewed.append)
        insp.artist_bank_toggle_requested.connect(banked.append)

        # Simulate clicking the rich-text artist link in the title
        insp._on_title_link("artist:fox_painter")
        self.assertEqual(viewed, ["fox_painter"])

        # Simulate clicking the + Bank Artist button
        insp.artist_row.btn_bank.click()
        self.assertEqual(banked, ["fox_painter"])

        insp.set_artists_banked({"fox_painter"})
        self.assertEqual(insp.artist_row.btn_bank.text(), "Artist In Bank")
        insp.deleteLater()

        # PreviewDialog clickable artist link & buttons
        dlg = PreviewDialog(post, all_posts=[post], artist_in_bank=lambda a: a == "fox_painter")
        self.assertEqual(dlg.side_artist_row.btn_bank.text(), "Artist In Bank")
        dlg_viewed = []
        dlg.artist_view.connect(dlg_viewed.append)
        dlg._on_info_link("artist:fox_painter")
        self.assertEqual(dlg_viewed, ["fox_painter"])
        dlg.close()
        dlg.deleteLater()

        # MainWindow end-to-end artist browsing and Spank Bank Artists sub-tab
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            s = Settings.load(tmp_path / "settings.json")
            w = MainWindow(s, data_dir=tmp_path)
            w.api.submit = lambda *args, **kwargs: None

            # Open artist view
            w._open_artist("Fox Painter", new_tab=False)
            self.assertEqual(w.current_tab.source.kind, "artist")
            self.assertEqual(w.current_tab.source.label, "fox_painter")
            self.assertFalse(w.current_tab.btn_bank_pool.isHidden())
            self.assertEqual(w.current_tab.btn_bank_pool.text(), "+ Bank Artist")

            # Toggle artist into Spank Bank via current tab's nav button
            w._toggle_current_pool_in_bank()
            self.assertTrue(w.spank_bank.has_artist("fox_painter"))
            self.assertEqual(w.current_tab.btn_bank_pool.text(), "Artist In Bank")
            self.assertEqual(w.bank_subtabs.tabText(2), "Artists (1)")
            self.assertEqual(w.bank_artists_list.count(), 1)

            # Add second artist via quick-add input in Spank Bank > Artists tab
            w.bank_artist_add_edit.setText("wolf_sketcher")
            w._add_bank_artist_from_edit()
            self.assertEqual(w.spank_bank.artist_count, 2)
            self.assertEqual(w.bank_subtabs.tabText(2), "Artists (2)")

            # Combined feed of all saved artists uses ~artist OR query
            w._open_bank_artists_feed(new_tab=False)
            self.assertEqual(w.current_tab.source.kind, "artist_feed")
            self.assertIn("~wolf_sketcher", w.current_tab.source.query)
            self.assertIn("~fox_painter", w.current_tab.source.query)

            w.close()

    def test_true_fullscreen_edge_clicks_and_spank_bank_downloads(self):
        import tempfile
        from PySide6.QtCore import QEvent, QPointF, Qt
        from PySide6.QtGui import QKeyEvent, QMouseEvent
        from PySide6.QtWidgets import QApplication
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings
        from e621cruiser.widgets import PreviewDialog

        p1 = make_post(id=801)
        p2 = make_post(id=802)
        p3 = make_post(id=803)
        dlg = PreviewDialog(p1, all_posts=[p1, p2, p3], index=0)

        # True fullscreen hides chrome and strips margins
        dlg.enter_fullscreen()
        self.assertTrue(dlg._true_fullscreen)
        self.assertTrue(dlg.top_bar_widget.isHidden())
        self.assertTrue(dlg.play_bar_widget.isHidden())
        self.assertTrue(dlg.right_panel.isHidden())
        self.assertEqual(dlg._main_layout.contentsMargins().left(), 0)

        # focusNextPrevChild is disabled so arrow keys never move button highlight
        self.assertFalse(dlg.focusNextPrevChild(True))
        self.assertFalse(dlg.focusNextPrevChild(False))

        # Sending Right / Left arrow events directly to child widgets still turns the page
        key_right = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_Right, Qt.KeyboardModifier.NoModifier)
        QApplication.sendEvent(dlg.next_btn, key_right)
        self.assertEqual(dlg.post.id, 802)

        key_left = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_Left, Qt.KeyboardModifier.NoModifier)
        QApplication.sendEvent(dlg.tag_list, key_left)
        self.assertEqual(dlg.post.id, 801)

        # Clicking right edge advances, clicking left edge goes back, center does nothing
        dlg.image_label.resize(1000, 800)
        right_click = QMouseEvent(
            QEvent.Type.MouseButtonPress, QPointF(920, 400), QPointF(920, 400),
            Qt.MouseButton.LeftButton, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier
        )
        QApplication.sendEvent(dlg.image_label, right_click)
        self.assertEqual(dlg.post.id, 802)

        center_click = QMouseEvent(
            QEvent.Type.MouseButtonPress, QPointF(500, 400), QPointF(500, 400),
            Qt.MouseButton.LeftButton, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier
        )
        QApplication.sendEvent(dlg.image_label, center_click)
        self.assertEqual(dlg.post.id, 802)

        left_click = QMouseEvent(
            QEvent.Type.MouseButtonPress, QPointF(50, 400), QPointF(50, 400),
            Qt.MouseButton.LeftButton, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier
        )
        QApplication.sendEvent(dlg.image_label, left_click)
        self.assertEqual(dlg.post.id, 801)

        # F11 toggles controls back on
        key_f11 = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_F11, Qt.KeyboardModifier.NoModifier)
        QApplication.sendEvent(dlg.image_label, key_f11)
        self.assertFalse(dlg._true_fullscreen)
        self.assertFalse(dlg.top_bar_widget.isHidden())

        dlg.close()
        dlg.deleteLater()

        # Test Spank Bank download actions
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            s = Settings.load(tmp_path / "settings.json")
            w = MainWindow(s, data_dir=tmp_path)
            enqueued: list[tuple[list[int], str]] = []
            w._enqueue = lambda posts, src=None: enqueued.append(
                ([p.id for p in posts], (src or w._source).kind)
            )
            pool = Pool(88, "Bank_Comic", [801, 802], category="series")
            w.api.submit = lambda func, *args, **kwargs: kwargs["on_result"]([p1, p2])

            w._toggle_spank_bank(p1)
            w._toggle_spank_bank(p2)
            w._toggle_pool_in_bank(pool)

            # Download all posts in Spank Bank
            w._download_spank_bank_posts()
            self.assertEqual(len(enqueued), 1)
            self.assertEqual(set(enqueued[-1][0]), {801, 802})
            self.assertEqual(enqueued[-1][1], "spank_bank")

            # Download selected post in Spank Bank
            w.bank_list.item(0).setSelected(True)
            w._download_selected_bank_posts()
            self.assertEqual(len(enqueued), 2)
            self.assertEqual(len(enqueued[-1][0]), 1)

            # Download all pools in Spank Bank
            w._download_all_bank_pools()
            self.assertEqual(len(enqueued), 3)
            self.assertEqual(enqueued[-1][0], [801, 802])
            self.assertEqual(enqueued[-1][1], "pool")

            # Download entire Spank Bank (posts + pools)
            enqueued.clear()
            w._download_entire_spank_bank()
            self.assertEqual(len(enqueued), 2)

            # Viewing Spank Bank shows "Download Spank Bank" button in ResultTab
            w._open_spank_bank(new_tab=False)
            self.assertFalse(w.current_tab.btn_dl_pool.isHidden())
            self.assertEqual(w.current_tab.btn_dl_pool.text(), "Download Spank Bank")

            w.close()


if __name__ == "__main__":
    unittest.main()
