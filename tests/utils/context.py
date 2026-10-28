import logging
import re

from playwright.sync_api import Page

from .backend import Backend
from .browser import Browser
from .config import FRONTEND_URL
from .config import Config
from .frontend import Dashboard
from .frontend import Frontend
from .moblin import Moblin
from .paypal import PayPal
from .stripe import Stripe
from .utils import manual_requirement

LOGGER = logging.getLogger(__name__)


class Context:
    """Everything the tests share: the backend, the frontend talking to it, a browser and Moblin."""

    def __init__(self, config: Config, backend: Backend, moblin: Moblin | None, interactive: bool):
        self.config = config
        self.backend = backend
        self.frontend = Frontend(backend.url)
        self.moblin = moblin
        self.interactive = interactive
        self.paypal = PayPal(backend.fake_paypal.url)
        self.stripe = Stripe(backend.fake_stripe.url)
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
            return self.frontend.fake_sign_in(login)
        return self._twitch_sign_in()

    def _twitch_sign_in(self) -> Dashboard:
        def job(page: Page) -> str:
            page.goto(f"{self.backend.url}/auth/twitch/login")
            page.wait_for_url(re.compile(re.escape(f"{FRONTEND_URL}/dashboard/")), timeout=300_000)
            return page.url

        if self.interactive:
            manual_requirement(LOGGER, "Sign in with Twitch in the opened browser if asked to")
        return self.frontend.dashboard_of(self.browser().run(job))

    def page_with_paypal(self, dashboard: Dashboard) -> dict:
        """Connects a PayPal seller to the signed in streamer's page, if not already."""
        account = dashboard.account()
        if account["settings"]["merchantId"] is not None:
            return account["settings"]
        self.paypal.onboard_seller(dashboard.connect_paypal())
        return dashboard.check_paypal()

    def page_with_stripe(self, dashboard: Dashboard) -> dict:
        """Gives the signed in streamer's page a Stripe account that takes payments, if not already."""
        account = dashboard.account()
        if account["settings"]["stripeAccountId"] is not None:
            return account["settings"]
        self.stripe.onboard_account(dashboard.connect_stripe())
        return dashboard.check_stripe()
