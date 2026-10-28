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
        self.assertEqual(w.grid.count(), 0)
        self.assertIn("Search for tags", w.result_label.text())
        w.close()

    def test_search_with_clicked_bool(self):
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings
        w = MainWindow(Settings())
        w.tag_edit.setText("wolf")
        # Simulate button clicked passing False as raw argument
        w._start_search(False)
        self.assertEqual(w._source.query, "wolf")
        w.close()

    def test_compose_query_filters(self):
        from e621cruiser.main_window import MainWindow
        from e621cruiser.settings import Settings
        w = MainWindow(Settings())
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


if __name__ == "__main__":
    unittest.main()
