import base64
import json
import logging
import time
from collections.abc import Callable

import requests
from playwright.sync_api import Page

from .browser import Browser
from .config import FRONTEND_URL
from .config import Config
from .utils import manual_confirmation

LOGGER = logging.getLogger(__name__)
API_URL = "https://api-m.sandbox.paypal.com"
TEST_CARD = {"number": "4111111111111111", "expiry": "2030-12", "security_code": "123", "name": "Test Viewer"}


class PayPalSandbox:
    """Talks to the PayPal sandbox as the partner, to pay orders and check what the backend did with them."""

    def __init__(self, config: Config):
        paypal = config.paypal()
        assert paypal is not None
        self._paypal = paypal
        self._token: str | None = None

    def order(self, order_id: str, merchant_id: str) -> dict:
        response = requests.get(
            f"{API_URL}/v2/checkout/orders/{order_id}",
            headers=self._headers(merchant_id),
            timeout=30,
        )
        response.raise_for_status()
        return response.json()

    def pay_with_card(self, order_id: str, merchant_id: str):
        """Approves the order with a sandbox test card, like a viewer paying in PayPal's popup."""
        response = requests.post(
            f"{API_URL}/v2/checkout/orders/{order_id}/confirm-payment-source",
            headers=self._headers(merchant_id),
            json={"payment_source": {"card": TEST_CARD}},
            timeout=60,
        )
        if not response.ok:
            raise Exception(f"Paying PayPal order {order_id} failed: {response.status_code} {response.text}")
        status = response.json()["status"]
        if status != "APPROVED":
            raise Exception(f"PayPal order {order_id} is {status} after paying, not APPROVED.")

    def _headers(self, merchant_id: str) -> dict:
        return {
            "authorization": f"Bearer {self._access_token()}",
            "paypal-auth-assertion": self._auth_assertion(merchant_id),
        }

    def _access_token(self) -> str:
        if self._token is None:
            response = requests.post(
                f"{API_URL}/v1/oauth2/token",
                auth=(self._paypal["client-id"], self._paypal["client-secret"]),
                data={"grant_type": "client_credentials"},
                timeout=30,
            )
            response.raise_for_status()
            self._token = response.json()["access_token"]
        return self._token

    def _auth_assertion(self, merchant_id: str) -> str:
        def encode(value: dict) -> str:
            return base64.b64encode(json.dumps(value).encode()).decode()

        return f"{encode({'alg': 'none'})}.{encode({'iss': self._paypal['client-id'], 'payer_id': merchant_id})}."


def _onboarded(url: str) -> bool:
    return "/unifiedonboarding/done" in url or url.startswith(f"{FRONTEND_URL}/dashboard/")


def _wait_for(page: Page, condition: Callable[[], bool], description: str, timeout: float = 60):
    end_time = time.monotonic() + timeout
    while not condition():
        if time.monotonic() > end_time:
            raise Exception(f"Timeout waiting for {description}.")
        page.wait_for_timeout(500)


def _onboard(page: Page, paypal: dict, action_url: str):
    """Signs the seller in and consents, unless PayPal remembers the seller is signed in already."""
    page.goto(action_url)
    email = page.locator("input[name=email]")
    _wait_for(page, lambda: _onboarded(page.url) or email.is_visible(), "PayPal to ask for the email")
    if not _onboarded(page.url):
        email.fill(paypal["seller-email"])
        page.locator("button[name=nextButton]").click()
        password = page.locator("#password")
        _wait_for(
            page, lambda: _onboarded(page.url) or password.is_visible(), "PayPal to ask for the password"
        )
        if not _onboarded(page.url):
            password.fill(paypal["seller-password"])
            page.locator("#btnLogin").click()
    _wait_for(page, lambda: _onboarded(page.url), "PayPal to finish connecting the seller", timeout=120)


def onboard_seller(browser: Browser, config: Config, action_url: str, interactive: bool):
    """Connects the sandbox seller through the PayPal sign up link, like a streamer in the dashboard."""
    paypal = config.paypal()
    assert paypal is not None

    def job(page: Page):
        try:
            _onboard(page, paypal, action_url)
        except Exception:
            if not interactive:
                raise
            if not _onboarded(page.url):
                manual_confirmation(
                    f"Connect the PayPal sandbox seller {paypal['seller-email']} in the opened browser."
                )

    LOGGER.debug("Connecting the PayPal sandbox seller %s", paypal["seller-email"])
    browser.run(job)
