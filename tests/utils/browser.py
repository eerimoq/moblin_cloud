import logging
import queue
import threading
import time
from collections.abc import Callable
from typing import TypeVar

from playwright.sync_api import Page
from playwright.sync_api import sync_playwright

from .config import FRONTEND_URL
from .config import Config
from .utils import CACHE_DIR
from .utils import FILES_DIR

LOGGER = logging.getLogger(__name__)
T = TypeVar("T")
USER_AGENT = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) "
    "Chrome/141.0.0.0 Safari/537.36"
)


class Browser:
    """A browser with a profile kept between runs, so PayPal and Twitch remember sign ins.

    Playwright objects may only be used by the thread that created them, and every test runs in
    its own thread, so the browser lives in a thread of its own that runs what tests ask it to.
    PayPal sends headless Chromium's user agent to its home page, so a desktop one is used.
    """

    def __init__(self, config: Config, interactive: bool):
        self._headless = config.headless_browser() and not interactive
        self._jobs: queue.Queue = queue.Queue()
        self._thread = threading.Thread(target=self._serve, daemon=True)
        self._thread.start()

    def close(self):
        self._jobs.put(None)
        self._thread.join(timeout=30)

    def run(self, job: Callable[[Page], T]) -> T:
        """Runs the job with a new page in the browser's thread and returns what it returns."""
        done = threading.Event()
        outcome: list = []
        self._jobs.put((job, done, outcome))
        done.wait()
        result, error = outcome
        if error is not None:
            raise error
        return result

    def _serve(self):
        with sync_playwright() as playwright:
            context = playwright.chromium.launch_persistent_context(
                str(CACHE_DIR / "browser"), headless=self._headless, user_agent=USER_AGENT
            )
            context.route(f"{FRONTEND_URL}/**", self._fulfill_frontend)
            try:
                while (item := self._jobs.get()) is not None:
                    job, done, outcome = item
                    page = context.new_page()
                    try:
                        outcome += [job(page), None]
                    except Exception as error:
                        self._save_screenshot(page)
                        outcome += [None, error]
                    finally:
                        page.close()
                        done.set()
            finally:
                context.close()

    def _save_screenshot(self, page: Page):
        path = FILES_DIR / f"browser-failure-{time.strftime('%H%M%S')}.png"
        try:
            page.screenshot(path=str(path), full_page=True)
            LOGGER.info("Browser failed at %s, screenshot in %s", page.url, path)
        except Exception as error:
            LOGGER.info("Browser failed at %s, no screenshot: %s", page.url, error)

    def _fulfill_frontend(self, route):
        route.fulfill(status=200, content_type="text/html", body="<html><body>Moblin</body></html>")
