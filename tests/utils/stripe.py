import logging
from collections.abc import Callable

import requests
from playwright.sync_api import Page

from .browser import Browser
from .config import FRONTEND_URL
from .config import Config
from .utils import manual_confirmation

LOGGER = logging.getLogger(__name__)
API_URL = "https://api.stripe.com"
TEST_CARD = "pm_card_visa"
TEST_DATA = (
    "The account is in the United States. Stripe's test values: SMS code 000-000, phone 0000000000, date of birth 1901-01-01, "
    "address line address_full_match, ID number 000000000, routing number 110000000 and "
    "account number 000123456789."
)


class StripeTestMode:
    """Talks to Stripe in test mode as the platform, to pay payments and check what the backend did with them."""

    def __init__(self, config: Config):
        stripe = config.stripe()
        assert stripe is not None
        self._secret_key = stripe["secret-key"]

    def payment(self, payment_id: str, account_id: str) -> dict:
        response = requests.get(
            f"{API_URL}/v1/payment_intents/{payment_id}",
            auth=(self._secret_key, ""),
            headers={"stripe-account": account_id},
            timeout=30,
        )
        response.raise_for_status()
        return response.json()

    def pay_with_card(self, payment_id: str, account_id: str):
        """Confirms the payment with a test card, like a viewer paying in Stripe's Payment Element."""
        response = requests.post(
            f"{API_URL}/v1/payment_intents/{payment_id}/confirm",
            auth=(self._secret_key, ""),
            headers={"stripe-account": account_id},
            data={"payment_method": TEST_CARD},
            timeout=60,
        )
        if not response.ok:
            raise Exception(
                f"Paying Stripe payment {payment_id} failed: {response.status_code} {response.text}"
            )
        status = response.json()["status"]
        if status != "requires_capture":
            raise Exception(f"Stripe payment {payment_id} is {status} after paying, not requires_capture.")


def onboard_account(browser: Browser, onboarding_url: Callable[[], str]):
    """Opens Stripe's onboarding of a new test connected account for the tester to fill in.

    Stripe's onboarding links work once, so another one is given for the tester's own browser,
    in case Stripe refuses the test browser.
    """

    def job(page: Page):
        page.goto(onboarding_url())
        manual_confirmation(
            "Set up the Stripe test account in the opened browser until it returns to Moblin, or, if "
            f"Stripe refuses that browser, open {onboarding_url()} in your own browser. " + TEST_DATA
        )
        if not page.url.startswith(f"{FRONTEND_URL}/dashboard/"):
            LOGGER.info("Stripe onboarding ended at %s, not the dashboard", page.url)

    LOGGER.debug("Setting up a Stripe test connected account")
    browser.run(job)
