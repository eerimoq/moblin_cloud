import requests

from .fake_stripe import SECRET_KEY
from .fake_stripe import TEST_CARD
from .website import follow_onboarding


class Stripe:
    """Talks to the fake Stripe as the platform, to pay payments and check what the backend did with them."""

    def __init__(self, url: str):
        self.url = url

    def payment(self, payment_id: str, account_id: str) -> dict:
        response = requests.get(
            f"{self.url}/v1/payment_intents/{payment_id}",
            auth=(SECRET_KEY, ""),
            headers={"stripe-account": account_id},
            timeout=30,
        )
        response.raise_for_status()
        return response.json()

    def pay_with_card(self, payment_id: str, account_id: str):
        """Confirms the payment with a test card, like a viewer paying in Stripe's Payment Element."""
        response = requests.post(
            f"{self.url}/v1/payment_intents/{payment_id}/confirm",
            auth=(SECRET_KEY, ""),
            headers={"stripe-account": account_id},
            data={"payment_method": TEST_CARD},
            timeout=30,
        )
        if not response.ok:
            raise Exception(
                f"Paying Stripe payment {payment_id} failed: {response.status_code} {response.text}"
            )
        status = response.json()["status"]
        if status != "requires_capture":
            raise Exception(f"Stripe payment {payment_id} is {status} after paying, not requires_capture.")

    def onboard_account(self, onboarding_url: str):
        """Sets the connected account up through Stripe's onboarding link, like a streamer in the dashboard."""
        follow_onboarding(onboarding_url)
