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
        w = MainWindow()
        self.assertEqual(w.grid.count(), 0)
        self.assertIn("Search for tags", w.result_label.text())
        w.close()

    def test_search_with_clicked_bool(self):
        from e621cruiser.main_window import MainWindow
        w = MainWindow()
        w.tag_edit.setText("wolf")
        # Simulate button clicked passing False as raw argument
        w._start_search(False)
        self.assertEqual(w._source.query, "wolf")
        w.close()


if __name__ == "__main__":
    unittest.main()
