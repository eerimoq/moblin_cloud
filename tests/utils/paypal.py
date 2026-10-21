import base64
import json
from urllib.parse import parse_qs
from urllib.parse import urlsplit

import requests

from .fake_paypal import CLIENT_ID
from .fake_paypal import CLIENT_SECRET
from .website import follow_onboarding

TEST_CARD = {"number": "4111111111111111", "expiry": "2030-12", "security_code": "123", "name": "Test Viewer"}


class PayPal:
    """Talks to the fake PayPal as the partner, to pay orders and check what the backend did with them."""

    def __init__(self, url: str):
        self.url = url
        self._token: str | None = None

    def order(self, order_id: str, merchant_id: str) -> dict:
        response = requests.get(
            f"{self.url}/v2/checkout/orders/{order_id}",
            headers=self._headers(merchant_id),
            timeout=30,
        )
        response.raise_for_status()
        return response.json()

    def pay_with_card(self, order_id: str, merchant_id: str):
        """Approves the order with a test card, like a viewer paying in PayPal's popup."""
        response = requests.post(
            f"{self.url}/v2/checkout/orders/{order_id}/confirm-payment-source",
            headers=self._headers(merchant_id),
            json={"payment_source": {"card": TEST_CARD}},
            timeout=30,
        )
        if not response.ok:
            raise Exception(f"Paying PayPal order {order_id} failed: {response.status_code} {response.text}")
        status = response.json()["status"]
        if status != "APPROVED":
            raise Exception(f"PayPal order {order_id} is {status} after paying, not APPROVED.")

    def onboard_seller(self, action_url: str) -> str:
        """Signs a new seller up through the PayPal sign up link, like a streamer in the dashboard.

        Returns the merchant ID.
        """
        returned = follow_onboarding(action_url)
        return parse_qs(urlsplit(returned).query)["merchantIdInPayPal"][0]

    def _headers(self, merchant_id: str) -> dict:
        return {
            "authorization": f"Bearer {self._access_token()}",
            "paypal-auth-assertion": self._auth_assertion(merchant_id),
        }

    def _access_token(self) -> str:
        if self._token is None:
            response = requests.post(
                f"{self.url}/v1/oauth2/token",
                auth=(CLIENT_ID, CLIENT_SECRET),
                data={"grant_type": "client_credentials"},
                timeout=30,
            )
            response.raise_for_status()
            self._token = response.json()["access_token"]
        return self._token

    def _auth_assertion(self, merchant_id: str) -> str:
        def encode(value: dict) -> str:
            return base64.b64encode(json.dumps(value).encode()).decode()

        return f"{encode({'alg': 'none'})}.{encode({'iss': CLIENT_ID, 'payer_id': merchant_id})}."
