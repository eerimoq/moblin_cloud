import logging
import re

from playwright.sync_api import Page

from .backend import Backend
from .browser import Browser
from .config import FRONTEND_URL
from .config import Config
from .moblin import Moblin
from .paypal import PayPalSandbox
from .paypal import onboard_seller
from .utils import manual_requirement
from .website import Dashboard
from .website import Website

LOGGER = logging.getLogger(__name__)


class Context:
    """Everything the tests share: the backend, the website talking to it, a browser and Moblin."""

    def __init__(self, config: Config, backend: Backend, moblin: Moblin | None, interactive: bool):
        self.config = config
        self.backend = backend
        self.website = Website(backend.url)
        self.moblin = moblin
        self.interactive = interactive
        self.paypal_sandbox = PayPalSandbox(config) if config.paypal() is not None else None
        self._browser: Browser | None = None

    def close(self):
        if self._browser is not None:
            self._browser.close()
            self._browser = None

    def browser(self) -> Browser:
        if self._browser is None:
            self._browser = Browser(self.config, self.interactive)
        return self._browser

    def sign_in(self, login: str = "streamer") -> Dashboard:
        if self.config.twitch_sign_in() == "fake":
            return self.website.fake_sign_in(login)
        return self._twitch_sign_in()

    def _twitch_sign_in(self) -> Dashboard:
        def job(page: Page) -> str:
            page.goto(f"{self.backend.url}/auth/twitch/login")
            page.wait_for_url(re.compile(re.escape(f"{FRONTEND_URL}/dashboard/")), timeout=300_000)
            return page.url

        if self.interactive:
            manual_requirement(LOGGER, "Sign in with Twitch in the opened browser if asked to")
        return self.website.dashboard_of(self.browser().run(job))

    def page_with_paypal(self, dashboard: Dashboard) -> dict:
        """Gives the signed in streamer a page and connects the sandbox seller to it, if not already."""
        account = dashboard.create_page()
        if account["settings"]["merchantId"] is not None:
            return account["settings"]
        action_url = dashboard.connect_paypal()
        onboard_seller(self.browser(), self.config, action_url, self.interactive)
        return dashboard.check_paypal()
