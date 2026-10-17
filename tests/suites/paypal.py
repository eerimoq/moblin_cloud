import time

import requests

from ..utils.context import Context
from ..utils.paypal import onboard_seller
from ..utils.test_case import TestCase
from ..utils.website import Dashboard
from .account import session_issued_at

TIPS = {"amounts": [500, 100, 100, 1000]}
SPIN = {"options": [{"spins": 3, "price": 500}, {"spins": 1, "price": 200}]}


class PayPalTestCase(TestCase):
    def setup(self):
        self.skip_if_no_paypal()


class PayPalConnect(PayPalTestCase):
    """Connect the streamer's PayPal account through PayPal's seller onboarding in the dashboard."""

    def run(self):
        dashboard = self.sign_in()
        settings = dashboard.create_page()["settings"]
        if settings["merchantId"] is not None:
            settings = dashboard.disconnect_paypal()
            self.assert_is_none(settings["merchantId"])
        action_url = dashboard.connect_paypal()
        self.assert_true(action_url.startswith("https://www.sandbox.paypal.com/"), action_url)
        self.assert_true(dashboard.account()["settings"]["paypalPending"])
        self.assert_backend_error(404, dashboard.check_paypal, "PayPal has not connected an account yet")
        onboard_seller(self.context.browser(), self.context.config, action_url, self.context.interactive)
        settings = dashboard.check_paypal()
        self.assert_is_not_none(settings["merchantId"])
        self.assert_false(settings["paypalPending"])
        self.assert_true(settings["paypalSandbox"])
        self.assert_equal(dashboard.check_paypal()["merchantId"], settings["merchantId"])


class PayPalConnectNeedsFreshSignIn(PayPalTestCase):
    """Ask the streamer to sign in again to connect PayPal when signed in more than 10 minutes ago."""

    def run(self):
        dashboard = self.sign_in()
        dashboard.create_page()
        stale = Dashboard(self.backend.url, session_issued_at(dashboard.session, int(time.time()) - 11 * 60))
        stale.account()
        self.assert_backend_error(403, stale.connect_paypal, "sign in again to connect PayPal")


class PayPalPaidSettings(PayPalTestCase):
    """Turn song requests, tips and the wheel on, which viewers see with what they need to pay with PayPal."""

    def run(self):
        dashboard = self.sign_in()
        settings = self.context.page_with_paypal(dashboard)
        merchant_id = settings["merchantId"]
        settings = dashboard.save_settings(currency="eur", paywall={"price": 150}, tips=TIPS, spin=SPIN)
        self.assert_equal(settings["currency"], "EUR")
        self.assert_equal(settings["paywall"], {"price": 150})
        self.assert_equal(settings["tips"], {"amounts": [100, 500, 1000]})
        self.assert_equal(
            settings["spin"]["options"], [{"spins": 1, "price": 200}, {"spins": 3, "price": 500}]
        )
        viewer = self.website.viewer(settings["id"])
        client_id = (self.context.config.paypal() or {})["client-id"]
        self.assert_equal(
            viewer.paywall(),
            {
                "price": 150,
                "currency": "EUR",
                "clientId": client_id,
                "merchantId": merchant_id,
                "environment": "sandbox",
            },
        )
        self.assert_equal(
            viewer.tips(),
            {
                "amounts": [100, 500, 1000],
                "currency": "EUR",
                "clientId": client_id,
                "merchantId": merchant_id,
                "environment": "sandbox",
            },
        )
        self.assert_equal(viewer.spin()["spin"]["options"], settings["spin"]["options"])
        settings = dashboard.save_settings(currency="USD")
        self.assert_equal([settings["paywall"], settings["tips"], settings["spin"]], [None, None, None])
        self.assert_equal(viewer.paywall(), None)
        self.assert_equal(viewer.tips(), None)
        self.assert_equal(viewer.spin()["spin"], None)


class PayPalInvalidPaidSettings(PayPalTestCase):
    """Refuse prices, tips and wheel options out of range."""

    def run(self):
        dashboard = self.sign_in()
        self.context.page_with_paypal(dashboard)
        paywall = "the paywall needs a price of 0.50 to 1000"
        tips = "tips need one to four amounts of 0.50 to 1000"
        spin = "the spin wheel needs one to four options of 1 to 10 spins for 0.50 to 1000"
        for changes, text in [
            ({"paywall": {"price": 49}}, paywall),
            ({"paywall": {"price": 100_001}}, paywall),
            ({"tips": {"amounts": []}}, tips),
            ({"tips": {"amounts": [100, 200, 300, 400, 500]}}, tips),
            ({"tips": {"amounts": [49]}}, tips),
            ({"spin": {"options": []}}, spin),
            ({"spin": {"options": [{"spins": 11, "price": 100}]}}, spin),
            ({"spin": {"options": [{"spins": 0, "price": 100}]}}, spin),
            ({"spin": {"options": [{"spins": 1, "price": 100}, {"spins": 1, "price": 200}]}}, spin),
            ({"spin": {"options": [{"spins": 1, "price": 10}]}}, spin),
        ]:
            self.assert_backend_error(400, lambda changes=changes: dashboard.save_settings(**changes), text)
        dashboard.save_settings(paywall={"price": 50}, tips={"amounts": [100_000]})
        dashboard.save_settings()


class PayPalDisconnect(PayPalTestCase):
    """Disconnect PayPal, which takes the paid features off the page until connected again."""

    def run(self):
        dashboard = self.sign_in()
        page_id = self.context.page_with_paypal(dashboard)["id"]
        dashboard.save_settings(paywall={"price": 100}, tips={"amounts": [100]})
        settings = dashboard.disconnect_paypal()
        self.assert_is_none(settings["merchantId"])
        self.assert_false(settings["paypalPending"])
        viewer = self.website.viewer(page_id)
        self.assert_equal(viewer.tips(), None)
        paywall = viewer.paywall()
        assert paywall is not None
        self.assert_is_none(paywall["merchantId"])
        self.assert_backend_error(
            503, lambda: viewer.order_tip(100), "Payments are unavailable right now. Try again later."
        )
        self.assert_backend_error(
            409, lambda: dashboard.save_settings(tips={"amounts": [200]}), "connect PayPal first"
        )
        dashboard.save_settings()
        self.context.page_with_paypal(dashboard)


class PayPalWebhookWithBadSignature(PayPalTestCase):
    """Refuse PayPal webhooks whose signature PayPal does not verify."""

    def run(self):
        event: dict = {
            "id": "WH-TEST",
            "event_type": "MERCHANT.PARTNER-CONSENT.REVOKED",
            "resource": {"merchant_id": "ABCDEFGHIJKLM"},
        }
        response = requests.post(
            f"{self.backend.url}/paypal/webhooks",
            json=event,
            headers={
                "paypal-auth-algo": "SHA256withRSA",
                "paypal-cert-url": "https://api.sandbox.paypal.com/v1/notifications/certs/CERT-360caa42-fca2a594-1d93a270",
                "paypal-transmission-id": "69cd13f0-d67a-11e5-baa3-778b53f4ae55",
                "paypal-transmission-sig": "bm90IGEgc2lnbmF0dXJl",
                "paypal-transmission-time": "2026-01-01T00:00:00Z",
            },
            timeout=60,
        )
        if "webhook-id" in (self.context.config.paypal() or {}):
            self.assert_equal(response.status_code, 400)
        else:
            self.assert_equal(response.status_code, 503)
        response = requests.post(f"{self.backend.url}/paypal/webhooks", data="not json", timeout=60)
        self.assert_in(response.status_code, [400, 503])


def tests(context: Context):
    return [
        PayPalConnect(context),
        PayPalConnectNeedsFreshSignIn(context),
        PayPalPaidSettings(context),
        PayPalInvalidPaidSettings(context),
        PayPalDisconnect(context),
        PayPalWebhookWithBadSignature(context),
    ]
