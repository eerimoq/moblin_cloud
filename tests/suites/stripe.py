import time
from urllib.parse import urlsplit

from ..utils.context import Context
from ..utils.test_case import TestCase
from ..utils.website import Dashboard
from .account import session_issued_at

TIPS = {"amounts": [500, 100]}


class StripeTestCase(TestCase):
    def setup(self):
        self.skip_if_no_stripe()


class StripeConnectStarts(StripeTestCase):
    """Start connecting Stripe in the dashboard, which waits for the streamer to set the account up on Stripe."""

    def run(self):
        dashboard = self.sign_in("stripepending")
        settings = dashboard.create_page()["settings"]
        self.assert_true(settings["stripe"])
        self.assert_true(settings["stripeTest"])
        self.assert_is_none(settings["stripeAccountId"])
        self.assert_false(settings["stripePending"])
        for country in [None, "XX", "IN"]:
            self.assert_backend_error(
                400,
                lambda country=country: dashboard.connect_stripe(country),
                "pick one of the countries Stripe supports",
            )
        self.assert_false(dashboard.account()["settings"]["stripePending"])
        onboarding_url = dashboard.connect_stripe()
        host = urlsplit(onboarding_url).netloc
        self.assert_true(host == "stripe.com" or host.endswith(".stripe.com"), onboarding_url)
        settings = dashboard.account()["settings"]
        self.assert_true(settings["stripePending"])
        self.assert_is_none(settings["stripeAccountId"])
        self.assert_backend_error(
            422, dashboard.check_stripe, "the Stripe account can't receive payments yet"
        )
        self.assert_true(urlsplit(dashboard.connect_stripe(None)).netloc.endswith("stripe.com"))
        self.assert_backend_error(
            409,
            lambda: dashboard.save_settings(tips=TIPS),
            "connect PayPal or Stripe first",
        )
        settings = dashboard.disconnect_stripe()
        self.assert_false(settings["stripePending"])
        self.assert_is_none(settings["stripeAccountId"])
        self.assert_true(dashboard.check_stripe()["stripe"])


class StripeConnectNeedsFreshSignIn(StripeTestCase):
    """Ask the streamer to sign in again to connect Stripe when signed in more than 10 minutes ago."""

    def run(self):
        dashboard = self.sign_in()
        dashboard.create_page()
        stale = Dashboard(self.backend.url, session_issued_at(dashboard.session, int(time.time()) - 11 * 60))
        stale.account()
        self.assert_backend_error(403, stale.connect_stripe, "sign in again to connect Stripe")


class StripeNotForPagesWithoutIt(StripeTestCase):
    """Refuse Stripe payments on a page whose streamer has not set up Stripe."""

    def run(self):
        dashboard = self.sign_in("stripepending")
        page_id = dashboard.create_page()["settings"]["id"]
        viewer = self.website.viewer(page_id)
        self.assert_equal(viewer.tips(), None)
        self.skip_if_no_paypal()
        self.context.page_with_paypal(dashboard)
        dashboard.save_settings(tips=TIPS)
        tips = viewer.tips()
        assert tips is not None
        self.assert_is_none(tips["stripe"])
        unavailable = "Payments are unavailable right now. Try again later."
        self.assert_backend_error(503, lambda: viewer.order_tip(100, provider="stripe"), unavailable)
        self.assert_backend_error(503, lambda: viewer.order_tip(100, provider="bitcoin"), unavailable)
        dashboard.save_settings()


class StripeConnect(StripeTestCase):
    """Set up a Stripe test account through Stripe's onboarding, which connects it to the page."""

    def run(self):
        dashboard = self.sign_in()
        settings = self.context.page_with_stripe(dashboard)
        self.assert_true(settings["stripeAccountId"].startswith("acct_"), settings["stripeAccountId"])
        self.assert_false(settings["stripePending"])
        self.assert_equal(dashboard.check_stripe()["stripeAccountId"], settings["stripeAccountId"])


class StripePaidSettings(StripeTestCase):
    """Turn tips on, which viewers see with what they need to pay with Stripe, and order a tip."""

    def run(self):
        dashboard = self.sign_in()
        settings = self.context.page_with_stripe(dashboard)
        account_id = settings["stripeAccountId"]
        dashboard.save_settings(currency="sek", tips=TIPS)
        viewer = self.website.viewer(settings["id"])
        tips = viewer.tips()
        assert tips is not None
        self.assert_equal(tips["amounts"], [100, 500])
        self.assert_equal(tips["currency"], "SEK")
        stripe = self.context.config.stripe() or {}
        self.assert_equal(
            tips["stripe"], {"publishableKey": stripe["publishable-key"], "accountId": account_id}
        )
        order = viewer.tip_order(500, "Viewer", "Hej!", "stripe")
        self.assert_true(order["orderId"].startswith("pi_"), order["orderId"])
        self.assert_true(order["clientSecret"].startswith(f"{order['orderId']}_secret_"))
        self.assert_equal([order["price"], order["currency"]], [500, "SEK"])
        assert self.context.stripe_test_mode is not None
        payment = self.context.stripe_test_mode.payment(order["orderId"], account_id)
        self.assert_equal(
            [payment["amount"], payment["currency"], payment["capture_method"], payment["status"]],
            [500, "sek", "manual", "requires_payment_method"],
        )
        self.assert_equal(payment["description"], "Tip from Viewer: Hej!")
        dashboard.save_settings(currency="USD")


def tests(context: Context):
    return [
        StripeConnectStarts(context),
        StripeConnectNeedsFreshSignIn(context),
        StripeNotForPagesWithoutIt(context),
        StripeConnect(context),
        StripePaidSettings(context),
    ]
