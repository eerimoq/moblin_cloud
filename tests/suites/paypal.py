import time

import requests

from ..utils.context import Context
from ..utils.frontend import Dashboard
from ..utils.test_case import TestCase
from .account import session_issued_at

TIPS = {"amounts": [500, 100, 100, 1000]}
SPIN = {"options": [{"spins": 3, "price": 500}, {"spins": 1, "price": 200}]}


def public_stripe(context: Context, settings: dict) -> dict | None:
    """What viewers get to pay with Stripe, when the streamer has set up Stripe too."""
    if settings["stripeAccountId"] is None:
        return None
    return {
        "publishableKey": context.backend.fake_stripe.config["publishable-key"],
        "accountId": settings["stripeAccountId"],
    }


class PayPalConnect(TestCase):
    """Connect the streamer's PayPal account through PayPal's seller onboarding in the dashboard."""

    def run(self):
        dashboard = self.sign_in()
        settings = dashboard.account()["settings"]
        if settings["merchantId"] is not None:
            settings = dashboard.disconnect_paypal()
            self.assert_is_none(settings["merchantId"])
        action_url = dashboard.connect_paypal()
        self.assert_true(action_url.startswith(f"{self.backend.fake_paypal.url}/"), action_url)
        self.assert_true(dashboard.account()["settings"]["paypalPending"])
        self.assert_backend_error(404, dashboard.check_paypal, "PayPal has not connected an account yet")
        self.context.paypal.onboard_seller(action_url)
        settings = dashboard.check_paypal()
        self.assert_is_not_none(settings["merchantId"])
        self.assert_false(settings["paypalPending"])
        self.assert_true(settings["paypalSandbox"])
        self.assert_equal(dashboard.check_paypal()["merchantId"], settings["merchantId"])


class PayPalConnectNeedsFreshSignIn(TestCase):
    """Ask the streamer to sign in again to connect PayPal when signed in more than 10 minutes ago."""

    def run(self):
        dashboard = self.sign_in()
        stale = Dashboard(self.backend.url, session_issued_at(dashboard.session, int(time.time()) - 11 * 60))
        stale.account()
        self.assert_backend_error(403, stale.connect_paypal, "sign in again to connect PayPal")


class PayPalPaidSettings(TestCase):
    """Turn tips and the wheel on, which viewers see with what they need to pay with PayPal, and change the currency only while they are off."""

    def run(self):
        dashboard = self.sign_in()
        settings = self.context.page_with_paypal(dashboard)
        merchant_id = settings["merchantId"]
        settings = dashboard.save_settings(currency="eur", tips=TIPS, spin=SPIN)
        self.assert_equal(settings["currency"], "EUR")
        self.assert_equal(settings["tips"], {"enabled": True, "amounts": [100, 500, 1000]})
        self.assert_equal(
            settings["spin"]["options"], [{"spins": 1, "price": 200}, {"spins": 3, "price": 500}]
        )
        viewer = self.frontend.viewer(settings["id"])
        client_id = self.backend.fake_paypal.config["client-id"]
        stripe = public_stripe(self.context, settings)
        self.assert_equal(
            viewer.tips(),
            {
                "amounts": [100, 500, 1000],
                "currency": "EUR",
                "clientId": client_id,
                "merchantId": merchant_id,
                "environment": "sandbox",
                "stripe": stripe,
            },
        )
        self.assert_equal(viewer.spin()["spin"]["options"], settings["spin"]["options"])
        self.assert_backend_error(
            409,
            lambda: dashboard.update_settings(currency="USD"),
            "turn off all tips before changing the currency",
        )
        tips = viewer.tips()
        assert tips is not None
        self.assert_equal(tips["currency"], "EUR")
        self.assert_equal(dashboard.save_settings()["currency"], "EUR")
        settings = dashboard.save_settings(currency="USD")
        self.assert_equal([settings["tips"], settings["spin"]], [None, None])
        self.assert_equal(viewer.tips(), None)
        self.assert_equal(viewer.spin()["spin"], None)


class PayPalInvalidPaidSettings(TestCase):
    """Refuse prices, tips and wheel options out of range."""

    def run(self):
        dashboard = self.sign_in()
        self.context.page_with_paypal(dashboard)
        tips = "tips need one to five amounts of 0.50 to 100000"
        spin = "the spin wheel needs one to five options of 1 to 10 spins for 0.50 to 100000"
        for changes, text in [
            ({"tips": {"amounts": []}}, tips),
            ({"tips": {"amounts": [100, 200, 300, 400, 500, 600]}}, tips),
            ({"tips": {"amounts": [49]}}, tips),
            ({"spin": {"options": []}}, spin),
            ({"spin": {"options": [{"spins": 11, "price": 100}]}}, spin),
            ({"spin": {"options": [{"spins": 0, "price": 100}]}}, spin),
            ({"spin": {"options": [{"spins": 1, "price": 10}]}}, spin),
        ]:
            self.assert_backend_error(400, lambda changes=changes: dashboard.save_settings(**changes), text)
        dashboard.save_settings(tips={"amounts": [50, 100_000]})
        dashboard.save_settings()


class PayPalDisconnect(TestCase):
    """Disconnect PayPal, which takes the paid features off the page until connected again."""

    def run(self):
        dashboard = self.sign_in()
        page_id = self.context.page_with_paypal(dashboard)["id"]
        dashboard.save_settings(tips={"amounts": [100]}, spin={"options": [{"spins": 1, "price": 100}]})
        settings = dashboard.disconnect_paypal()
        self.assert_is_none(settings["merchantId"])
        self.assert_false(settings["paypalPending"])
        viewer = self.frontend.viewer(page_id)
        self.assert_equal(viewer.tips(), None)
        self.assert_equal(viewer.spin()["spin"], None)
        self.assert_backend_error(
            503, lambda: viewer.order_tip(100), "Tips are unavailable right now. Try again later."
        )
        self.assert_backend_error(
            409, lambda: dashboard.save_settings(tips={"amounts": [200]}), "connect PayPal or Stripe first"
        )
        dashboard.save_settings()
        self.context.page_with_paypal(dashboard)


def post_webhook(backend_url: str, event_type: str, merchant_id: str, signature: str) -> int:
    """Posts a webhook event like PayPal, whose signature the fake PayPal verifies when it is "good"."""
    response = requests.post(
        f"{backend_url}/paypal/webhooks",
        json={"id": "WH-TEST", "event_type": event_type, "resource": {"merchant_id": merchant_id}},
        headers={
            "paypal-auth-algo": "SHA256withRSA",
            "paypal-cert-url": "https://api.sandbox.paypal.com/v1/notifications/certs/CERT-360caa42-fca2a594-1d93a270",
            "paypal-transmission-id": "69cd13f0-d67a-11e5-baa3-778b53f4ae55",
            "paypal-transmission-sig": signature,
            "paypal-transmission-time": "2026-01-01T00:00:00Z",
        },
        timeout=60,
    )
    return response.status_code


class PayPalWebhookWithBadSignature(TestCase):
    """Refuse PayPal webhooks whose signature PayPal does not verify."""

    def run(self):
        dashboard = self.sign_in("webhookbadsignature")
        merchant_id = self.context.page_with_paypal(dashboard)["merchantId"]
        status = post_webhook(
            self.backend.url, "MERCHANT.PARTNER-CONSENT.REVOKED", merchant_id, "bm90IGEgc2lnbmF0dXJl"
        )
        self.assert_equal(status, 400)
        self.assert_equal(dashboard.account()["settings"]["merchantId"], merchant_id)
        response = requests.post(f"{self.backend.url}/paypal/webhooks", data="not json", timeout=60)
        self.assert_equal(response.status_code, 400)


class PayPalWebhooks(TestCase):
    """Connect PayPal when PayPal says the onboarding completed, and disconnect it when the streamer revokes the consent."""

    def run(self):
        dashboard = self.sign_in("paypalwebhooks")
        merchant_id = self.context.paypal.onboard_seller(dashboard.connect_paypal())
        self.assert_true(dashboard.account()["settings"]["paypalPending"])
        self.assert_equal(
            post_webhook(self.backend.url, "MERCHANT.ONBOARDING.COMPLETED", merchant_id, "good"), 200
        )
        settings = dashboard.account()["settings"]
        self.assert_equal([settings["merchantId"], settings["paypalPending"]], [merchant_id, False])
        self.assert_equal(
            post_webhook(self.backend.url, "MERCHANT.PARTNER-CONSENT.REVOKED", merchant_id, "good"), 200
        )
        self.assert_is_none(dashboard.account()["settings"]["merchantId"])


def tests(context: Context):
    return [
        PayPalConnect(context),
        PayPalConnectNeedsFreshSignIn(context),
        PayPalPaidSettings(context),
        PayPalInvalidPaidSettings(context),
        PayPalDisconnect(context),
        PayPalWebhookWithBadSignature(context),
        PayPalWebhooks(context),
    ]
