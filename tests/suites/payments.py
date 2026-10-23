import logging
import time
from collections.abc import Callable

from ..utils.context import Context
from ..utils.fake_moblin import FakeMoblin
from ..utils.moblin import Moblin
from ..utils.moblin import wheel_option
from ..utils.test_case import TestCase
from ..utils.website import BackendError

LOGGER = logging.getLogger(__name__)
TIPS = {"amounts": [100, 700]}
SPIN = {"options": [{"spins": 1, "price": 100}, {"spins": 3, "price": 250}]}
WHEEL = [wheel_option("Push-ups"), wheel_option("Sing a song", 2), wheel_option("Nothing", 3)]


class PaymentTestCase(TestCase):
    """A viewer paying with PayPal or Stripe on the page of the signed in streamer.

    With PayPal a seller of the fake PayPal is connected, with Stripe an account of the fake Stripe.
    """

    def __init__(self, context: Context, provider: str = "paypal"):
        super().__init__(context, f"{type(self).__name__}Stripe" if provider == "stripe" else None)
        self.provider = provider

    def setup(self):
        self.dashboard = self.sign_in()
        if self.provider == "stripe":
            self.settings = self.context.page_with_stripe(self.dashboard)
            self.account_id = self.settings["stripeAccountId"]
        else:
            self.settings = self.context.page_with_paypal(self.dashboard)
            self.merchant_id = self.settings["merchantId"]
        self.viewer = self.website.viewer(self.settings["id"])
        self.dashboard.save_settings()
        self.dashboard.save_settings(currency="USD")

    def teardown(self):
        if hasattr(self, "dashboard"):
            self.dashboard.save_settings()
            self.dashboard.save_settings(currency="USD")

    @property
    def moblin(self) -> Moblin:
        assert self.context.moblin is not None
        return self.context.moblin

    def connect_moblin(self, wheel: list[dict] | None = None):
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key(), wheel)
        self.wait_until(lambda: self.dashboard.account()["settings"]["connected"], "Moblin to be connected")

    def order_tip(self, amount: int, user: str | None = None, message: str | None = None) -> str:
        return self.viewer.order_tip(amount, user, message, self.provider)

    def order_spin(self, spins: int, user: str | None = None, message: str | None = None) -> str:
        return self.viewer.order_spin(spins, user, message, self.provider)

    def pay(self, order_id: str):
        """Pays the order with a test card, like the viewer in PayPal's popup or Stripe's Payment Element."""
        if self.provider == "stripe":
            self.context.stripe.pay_with_card(order_id, self.account_id)
        else:
            self.context.paypal.pay_with_card(order_id, self.merchant_id)

    def capture(self, capture: Callable[[str], dict], order_id: str) -> dict:
        """Captures like the page, trying again when the payment provider is temporarily unavailable."""
        for attempt in range(3):
            try:
                return capture(order_id)
            except BackendError as error:
                if error.status != 502 or attempt == 2:
                    raise
                LOGGER.debug("Capturing %s failed with %s, trying again", order_id, error)
                time.sleep(5)
        raise AssertionError("unreachable")

    def assert_paid(self, order_id: str, value: str, currency: str = "USD"):
        if self.provider == "stripe":
            payment = self.stripe_payment(order_id)
            self.assert_equal(payment["status"], "succeeded")
            self.assert_equal(
                [payment["currency"], payment["amount_received"]],
                [currency.lower(), round(float(value) * 100)],
            )
            return
        order = self.context.paypal.order(order_id, self.merchant_id)
        self.assert_equal(order["status"], "COMPLETED")
        unit = order["purchase_units"][0]
        self.assert_equal(unit["payee"]["merchant_id"], self.merchant_id)
        self.assert_equal([unit["amount"]["currency_code"], unit["amount"]["value"]], [currency, value])
        capture = unit["payments"]["captures"][0]
        self.assert_equal(capture["status"], "COMPLETED")

    def assert_not_paid(self, order_id: str):
        if self.provider == "stripe":
            self.assert_not_equal(self.stripe_payment(order_id)["status"], "succeeded")
            return
        order = self.context.paypal.order(order_id, self.merchant_id)
        self.assert_not_equal(order["status"], "COMPLETED")

    def assert_hold_released(self, order_id: str):
        """Stripe has put the money on hold on the viewer's card, which must be released."""
        if self.provider == "stripe":
            self.assert_equal(self.stripe_payment(order_id)["status"], "canceled")
        else:
            self.assert_not_paid(order_id)

    def stripe_payment(self, payment_id: str) -> dict:
        """The payment on the streamer's Stripe account, which is where it must be."""
        return self.context.stripe.payment(payment_id, self.account_id)


class PaymentsTipNotApproved(PaymentTestCase):
    """Refuse to capture a tip the viewer never paid, and captures of the order as something else."""

    def run(self):
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.order_tip(700, "Viewer", "Hi!")
        self.assert_backend_error(
            402, lambda: self.capture(self.viewer.capture_tip, order_id), "The payment was not completed."
        )
        gone = "This payment is unknown or expired, so no money was taken."
        self.assert_backend_error(410, lambda: self.capture(self.viewer.capture_spin, order_id), gone)
        other = self.website.viewer("AAAAAAAAAAAAAAAA")
        self.assert_backend_error(410, lambda: other.capture_tip(order_id), gone)
        self.assert_backend_error(410, lambda: self.viewer.capture_tip("UNKNOWN123"), gone)
        self.assert_backend_error(410, lambda: self.viewer.capture_tip("pi_unknown"), gone)
        self.assert_not_paid(order_id)


class PaymentsInvalidTip(PaymentTestCase):
    """Refuse tips below the lowest amount, with too long messages, or when tips are off."""

    def run(self):
        self.assert_backend_error(
            400, lambda: self.order_tip(100), "The streamer no longer takes tips. Reload the page."
        )
        self.dashboard.save_settings(tips=TIPS)
        self.assert_backend_error(400, lambda: self.order_tip(99), "Tip at least 1.00.")
        self.assert_backend_error(
            400,
            lambda: self.order_tip(100, message=201 * "x"),
            "The message can be at most 200 characters.",
        )
        self.order_tip(333, message=200 * "x")
        self.order_tip(100_001)


class PaymentsTipPaid(PaymentTestCase):
    """Tip, which pays the streamer's PayPal or Stripe account and shows the tip in Moblin."""

    def run(self):
        if self.context.moblin is not None:
            self.connect_moblin()
        self.dashboard.save_settings(currency="EUR", tips=TIPS)
        tips = self.viewer.tips()
        assert tips is not None
        self.assert_equal(tips["currency"], "EUR")
        order_id = self.order_tip(700, "  Viewer  ", "  Great stream!  ")
        self.pay(order_id)
        self.assert_equal(self.capture(self.viewer.capture_tip, order_id), {})
        self.assert_paid(order_id, "7.00", "EUR")
        self.assert_equal(self.capture(self.viewer.capture_tip, order_id), {})
        if self.context.moblin is None:
            return
        tip = self.moblin.wait_for_backend_request("tip", order_id)
        self.assert_equal(
            tip.body,
            {
                "name": "Viewer",
                "message": "Great stream!",
                "amount": "7.00",
                "currency": "EUR",
                "requestId": order_id,
            },
        )


class PaymentsAnonymousTipPaid(PaymentTestCase):
    """Tip without a name or message."""

    def run(self):
        self.skip_if_no_moblin()
        self.connect_moblin()
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.order_tip(100)
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        tip = self.moblin.wait_for_backend_request("tip", order_id)
        self.assert_equal([tip.body["name"], tip.body["message"]], ["Someone", ""])


class PaymentsTipDeliveredWhenMoblinConnects(PaymentTestCase):
    """Take a tip while Moblin is offline and show it in Moblin when it connects."""

    def run(self):
        self.skip_if_no_moblin()
        self.moblin.disconnect_from_backend()
        self.dashboard.save_settings(tips=TIPS)
        self.assert_false(self.dashboard.account()["settings"]["connected"])
        order_id = self.order_tip(100, "Early")
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        self.assert_paid(order_id, "1.00")
        self.connect_moblin()
        tip = self.moblin.wait_for_backend_request("tip", order_id)
        self.assert_equal(tip.body["name"], "Early")


class PaymentsTipDeliveredAfterBackendRestart(PaymentTestCase):
    """Keep a tip that Moblin has not got yet when the backend restarts, and deliver it afterwards."""

    def run(self):
        self.skip_if_no_moblin()
        self.moblin.disconnect_from_backend()
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.order_tip(700, "Patient")
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        self.backend.restart()
        self.connect_moblin()
        self.moblin.wait_for_backend_request("tip", order_id)


class PaymentsTipToEveryDevice(PaymentTestCase):
    """Keep sending a tip to a connected device that has not got it, after another device got it."""

    def run(self):
        self.skip_if_no_moblin()
        self.connect_moblin()
        self.dashboard.save_settings(tips=TIPS)
        self.second = FakeMoblin(
            None, self.backend.url, key=self.dashboard.moblin_cloud_key(), device_id="REFUSING"
        )
        self.second.refuse_tips = True
        self.second.start()
        self.wait_until(
            lambda: sum(device["connected"] for device in self.dashboard.account()["devices"]) == 2,
            "the second device to connect",
        )
        order_id = self.order_tip(100, "Everyone")
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        self.moblin.wait_for_backend_request("tip", order_id)

        def tip() -> dict:
            return self.dashboard.recent("tip")["items"][0]

        self.wait_until(
            lambda: any(device["received"] for device in tip()["devices"]), "the first device to get the tip"
        )
        self.assert_equal(tip()["status"], "waiting")
        self.assert_equal(sorted(device["received"] for device in tip()["devices"]), [False, True])
        self.second.refuse_tips = False
        self.wait_until(
            lambda: order_id in self.second.handled_request_ids, "the second device to get the tip"
        )
        self.wait_until(lambda: tip()["status"] == "delivered", "the tip to be delivered")
        self.assert_equal([device["received"] for device in tip()["devices"]], [True, True])

    def teardown(self):
        if hasattr(self, "second"):
            self.second.stop()
        super().teardown()


class PaymentsTipSentAgain(PaymentTestCase):
    """Send a delivered tip to Moblin again from the dashboard, under a new request id so Moblin shows it again."""

    def run(self):
        self.skip_if_no_moblin()
        self.connect_moblin()
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.order_tip(700, "Again", "Once more")
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        first = self.moblin.wait_for_backend_request("tip", order_id)
        self.wait_until(
            lambda: self.dashboard.recent("tip")["items"][0]["status"] == "delivered",
            "the tip to be delivered",
        )
        for resends in [1, 2]:
            self.assert_equal(self.dashboard.resend(order_id), {})
            again = self.moblin.wait_for_backend_request("tip", f"{order_id}-{resends}")
            self.assert_equal(
                {**again.body, "requestId": order_id},
                first.body,
            )
            self.wait_until(
                lambda: self.dashboard.recent("tip")["items"][0]["status"] == "delivered",
                "the tip to be delivered again",
            )
        self.assert_equal([item["id"] for item in self.dashboard.recent("tip")["items"]].count(order_id), 1)
        self.assert_backend_error(404, lambda: self.dashboard.resend("UNKNOWN123"))


class PaymentsSpinNeedsWheel(PaymentTestCase):
    """Refuse wheel spins when the wheel is off in the dashboard or Moblin has no wheel."""

    def run(self):
        self.skip_if_no_moblin()
        self.connect_moblin()
        self.assert_backend_error(
            400, lambda: self.order_spin(1), "The streamer turned the wheel off. Reload the page."
        )
        self.dashboard.save_settings(spin=SPIN)
        self.assert_equal(self.viewer.spin()["wheel"], None)
        self.assert_backend_error(
            409, lambda: self.order_spin(1), "The streamer is not taking wheel spins right now."
        )
        self.moblin.disconnect_from_backend()
        self.wait_until(lambda: not self.viewer.spin()["connected"], "Moblin to be disconnected")
        self.assert_backend_error(
            404,
            lambda: self.order_spin(1),
            "The streamer's wheel is not available right now. Try again when they are live.",
        )


class PaymentsSpinPaid(PaymentTestCase):
    """Buy wheel spins, which spins the wheel in Moblin."""

    def run(self):
        self.skip_if_no_moblin()
        self.connect_moblin(WHEEL)
        self.dashboard.save_settings(spin=SPIN)
        self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "the wheel to be on the page")
        spin = self.viewer.spin()
        self.assert_equal(
            spin["wheel"],
            {"options": [{"text": option["text"], "weight": option["weight"]} for option in WHEEL]},
        )
        self.assert_equal(spin["spin"]["options"], SPIN["options"])
        self.assert_backend_error(
            400, lambda: self.order_spin(2), "The streamer changed the prices. Reload the page."
        )
        order_id = self.order_spin(3, "Spinner", "Spin it!")
        self.pay(order_id)
        self.assert_equal(self.capture(self.viewer.capture_spin, order_id), {})
        self.assert_paid(order_id, "2.50")
        request = self.moblin.wait_for_backend_request("spinWheelOfLuck", order_id)
        self.assert_equal(
            request.body,
            {"name": "Spinner", "message": "Spin it!", "count": 3, "requestId": order_id},
        )


class PaymentsSpinWhenWheelIsGone(PaymentTestCase):
    """Do not take the money for spins when Moblin's wheel is gone before the capture."""

    def run(self):
        self.skip_if_no_moblin()
        self.connect_moblin(WHEEL)
        self.dashboard.save_settings(spin=SPIN)
        self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "the wheel to be on the page")
        order_id = self.order_spin(1)
        self.pay(order_id)
        self.connect_moblin()
        self.wait_until(lambda: self.viewer.spin()["wheel"] is None, "the wheel to be gone")
        self.assert_backend_error(
            404,
            lambda: self.capture(self.viewer.capture_spin, order_id),
            "The streamer's wheel is gone, so your payment was not taken.",
        )
        self.assert_hold_released(order_id)


def tests(context: Context):
    return [
        test(context, provider)
        for provider in ["paypal", "stripe"]
        for test in [
            PaymentsTipNotApproved,
            PaymentsInvalidTip,
            PaymentsTipPaid,
            PaymentsAnonymousTipPaid,
            PaymentsTipDeliveredWhenMoblinConnects,
            PaymentsTipDeliveredAfterBackendRestart,
            PaymentsTipToEveryDevice,
            PaymentsTipSentAgain,
            PaymentsSpinNeedsWheel,
            PaymentsSpinPaid,
            PaymentsSpinWhenWheelIsGone,
        ]
    ]
