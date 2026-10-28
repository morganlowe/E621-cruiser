"""Background execution helpers that deliver results on the Qt main thread."""
from __future__ import annotations

import threading
from typing import Any, Callable

import requests
from PySide6.QtCore import QObject, QThreadPool, Signal
from PySide6.QtGui import QImage

from .api import user_agent


class Runner(QObject):
    """Runs callables on a thread pool; callbacks fire on the GUI thread."""

    _finished = Signal(object, object)  # callback, value

    def __init__(self, max_threads: int, parent: QObject | None = None):
        super().__init__(parent)
        self.pool = QThreadPool(self)
        self.pool.setMaxThreadCount(max_threads)
        self._stopped = False
        self._lock = threading.Lock()
        self._finished.connect(self._dispatch)

    def submit(
        self,
        fn: Callable[..., Any],
        *args: Any,
        on_result: Callable[[Any], None] | None = None,
        on_error: Callable[[Exception], None] | None = None,
    ) -> None:
        if self._stopped:
            return

        def job() -> None:
            if self._stopped:
                return
            try:
                value = fn(*args)
            except Exception as e:  # noqa: BLE001 - surfaced to the UI
                if not self._stopped:
                    try:
                        self._finished.emit(on_error, e)
                    except RuntimeError:
                        pass
            else:
                if not self._stopped:
                    try:
                        self._finished.emit(on_result, value)
                    except RuntimeError:
                        pass

        self.pool.start(job)

    def clear_pending(self) -> None:
        self.pool.clear()

    def stop(self) -> None:
        self._stopped = True
        self.pool.clear()
        try:
            self._finished.disconnect()
        except RuntimeError:
            pass

    @staticmethod
    def _dispatch(cb: Callable[[Any], None] | None, value: Any) -> None:
        if cb is not None:
            try:
                cb(value)
            except RuntimeError:
                pass


_local = threading.local()


def http_session() -> requests.Session:
    """Per-thread session for static file / thumbnail requests."""
    s = getattr(_local, "session", None)
    if s is None:
        s = requests.Session()
        s.headers["User-Agent"] = user_agent()
        _local.session = s
    return s


def fetch_image(url: str, timeout: int = 15) -> QImage:
    r = http_session().get(url, timeout=timeout)
    r.raise_for_status()
    img = QImage()
    img.loadFromData(r.content)
    return img
