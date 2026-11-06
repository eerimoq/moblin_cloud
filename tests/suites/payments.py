import logging
import time
from collections.abc import Callable

from ..utils.context import Context
from ..utils.fake_moblin import FakeMoblin
from ..utils.fake_paypal import FakePayPal
from ..utils.fake_stripe import FakeStripe
from ..utils.frontend import BackendError
from ..utils.moblin import Moblin
from ..utils.moblin import wheel_option
from ..utils.test_case import TestCase

LOGGER = logging.getLogger(__name__)
TIPS = {"amounts": [100, 700]}
SPIN = {"options": [{"spins": 1, "price": 100}, {"spins": 3, "price": 250}]}
WHEEL = [wheel_option("Push-ups"), wheel_option("Sing a song", 2), wheel_option("Nothing", 3)]
DEADLINE = ["--payment-deadline", "2"]


def wait_past_deadline():
    time.sleep(3.1)


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
        self.viewer = self.frontend.viewer(self.settings["id"])
        self.dashboard.save_settings()
        self.dashboard.save_settings(currency="USD")

    def teardown(self):
        self.backend.fake_paypal.reset()
        self.backend.fake_stripe.reset()
        if hasattr(self, "dashboard"):
            self.dashboard.save_settings()
            self.dashboard.save_settings(currency="USD")

    @property
    def moblin(self) -> Moblin:
        return self.context.moblin

    @property
    def fake(self) -> FakePayPal | FakeStripe:
        return self.backend.fake_stripe if self.provider == "stripe" else self.backend.fake_paypal

    @property
    def provider_name(self) -> str:
        return "Stripe" if self.provider == "stripe" else "PayPal"

    def no_answer(self, kept: str = "the tip will still be shown") -> str:
        return f"{self.provider_name} did not answer. If the money was taken, {kept} or the money refunded."

    def paid_tip(self, amount: int, user: str) -> str:
        order_id = self.order_tip(amount, user)
        self.pay(order_id)
        return order_id

    def received(self, order_id: str) -> bool:
        return order_id in [item["id"] for item in self.dashboard.recent("tip")["items"]]

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
            402, lambda: self.capture(self.viewer.capture_tip, order_id), "The tip was not completed."
        )
        gone = "This tip is unknown or expired, so no money was taken."
        self.assert_backend_error(410, lambda: self.capture(self.viewer.capture_spin, order_id), gone)
        other = self.frontend.viewer("AAAAAAAAAAAAAAAA")
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
        tip = self.moblin.wait_for_backend_request("message", order_id)
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
        self.connect_moblin()
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.order_tip(100)
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        tip = self.moblin.wait_for_backend_request("message", order_id)
        self.assert_equal([tip.body["name"], tip.body["message"]], ["Someone", ""])


class PaymentsTipDeliveredWhenMoblinConnects(PaymentTestCase):
    """Take a tip while Moblin is offline and show it in Moblin when it connects."""

    def run(self):
        self.moblin.disconnect_from_backend()
        self.dashboard.save_settings(tips=TIPS)
        self.assert_false(self.dashboard.account()["settings"]["connected"])
        order_id = self.order_tip(100, "Early")
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        self.assert_paid(order_id, "1.00")
        self.connect_moblin()
        tip = self.moblin.wait_for_backend_request("message", order_id)
        self.assert_equal(tip.body["name"], "Early")


class PaymentsTipDeliveredAfterBackendRestart(PaymentTestCase):
    """Keep a tip that Moblin has not got yet when the backend restarts, and deliver it afterwards."""

    def run(self):
        self.moblin.disconnect_from_backend()
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.order_tip(700, "Patient")
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        self.backend.restart()
        self.connect_moblin()
        self.moblin.wait_for_backend_request("message", order_id)


class PaymentsTipToEveryDevice(PaymentTestCase):
    """Keep sending a tip to a connected device that has not got it, after another device got it."""

    def run(self):
        self.connect_moblin()
        self.dashboard.save_settings(tips=TIPS)
        self.second = FakeMoblin(
            None,
            self.backend.url,
            key=self.dashboard.moblin_cloud_key(),
            device_id="REFUSING",
            reconnect_delay=0.1,
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
        self.moblin.wait_for_backend_request("message", order_id)

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
        if hasattr(self, "dashboard"):
            self.dashboard.delete_account()


class PaymentsTipSentAgain(PaymentTestCase):
    """Send a delivered tip to Moblin again from the dashboard, under a new request id so Moblin shows it again."""

    def run(self):
        self.connect_moblin()
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.order_tip(700, "Again", "Once more")
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        first = self.moblin.wait_for_backend_request("message", order_id)
        self.wait_until(
            lambda: self.dashboard.recent("tip")["items"][0]["status"] == "delivered",
            "the tip to be delivered",
        )
        for resends in [1, 2]:
            self.assert_equal(self.dashboard.resend(order_id), {})
            again = self.moblin.wait_for_backend_request("message", f"{order_id}-{resends}")
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
        request = self.moblin.wait_for_backend_request("wheelOfLuck", order_id)
        self.assert_equal(
            request.body,
            {"name": "Spinner", "message": "Spin it!", "count": 3, "requestId": order_id},
        )


class PaymentsSpinWhenWheelIsGone(PaymentTestCase):
    """Do not take the money for spins when Moblin's wheel is gone before the capture."""

    def run(self):
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
            "The streamer's wheel is gone, so your tip was not taken.",
        )
        self.assert_hold_released(order_id)


class PaymentsTipDeclined(PaymentTestCase):
    """Tell the viewer when PayPal or Stripe declines the payment, and show no tip."""

    def run(self):
        self.dashboard.save_settings(tips=TIPS)
        for outcome in ["declined"] if self.provider == "stripe" else ["declined", "refused"]:
            order_id = self.paid_tip(100, "Declined")
            self.fake.plan_capture(order_id, outcome)
            self.assert_backend_error(
                402,
                lambda order_id=order_id: self.viewer.capture_tip(order_id),
                f"{self.provider_name} declined the payment.",
            )
            self.assert_not_paid(order_id)
            self.assert_false(self.received(order_id))


class PaymentsTipUnderReview(PaymentTestCase):
    """Tell the viewer when PayPal or Stripe is still reviewing the payment, and show the tip once the review ends with the money taken."""

    def run(self):
        self.connect_moblin()
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.paid_tip(700, "Reviewed")
        self.fake.plan_capture(order_id, "pending")
        name = self.provider_name
        reviewing = (
            f"{name} is still reviewing the tip. If {name} approves it within half an hour, the tip "
            "will be shown and the money reaches the streamer, otherwise the money is refunded."
        )
        for _ in range(2):
            self.assert_backend_error(402, lambda: self.viewer.capture_tip(order_id), reviewing)
        self.assert_false(self.received(order_id))
        self.fake.complete(order_id)
        self.assert_equal(self.viewer.capture_tip(order_id), {})
        self.assert_paid(order_id, "7.00")
        tip = self.moblin.wait_for_backend_request("message", order_id)
        self.assert_equal(tip.body["name"], "Reviewed")


class PaymentsSpinAndEffectUnderReview(PaymentTestCase):
    """Tell the viewer that the wheel is not spun and the effect not played yet while PayPal or Stripe reviews the payment."""

    def run(self):
        self.connect_moblin(WHEEL)
        self.dashboard.change_settings(spin=SPIN, effects={"price": 100})
        self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "the wheel to be on the page")
        name = self.provider_name
        spin = self.order_spin(1)
        effect = self.viewer.order_effect("fart", 100, provider=self.provider)
        for order_id in [spin, effect]:
            self.pay(order_id)
            self.fake.plan_capture(order_id, "pending")
        self.assert_backend_error(
            402,
            lambda: self.viewer.capture_spin(spin),
            f"{name} is still reviewing the tip, so the wheel was not spun yet. It will be spun if "
            f"{name} approves it within half an hour, otherwise the money is refunded.",
        )
        self.assert_backend_error(
            402,
            lambda: self.viewer.capture_effect(effect),
            f"{name} is still reviewing the tip, so the effect was not played yet. It will be played if "
            f"{name} approves it within half an hour, otherwise the money is refunded.",
        )
        self.fake.complete(spin)
        self.assert_equal(self.viewer.capture_spin(spin), {})
        self.moblin.wait_for_backend_request("wheelOfLuck", spin)

    def teardown(self):
        if hasattr(self, "dashboard"):
            self.dashboard.change_settings(spin=None, effects=None)
        super().teardown()


class PaymentsTipAnswerLost(PaymentTestCase):
    """Show a tip whose money was taken although the answer to the capture was lost, and take the money once when the viewer tries a failed capture again."""

    def run(self):
        self.connect_moblin()
        self.dashboard.save_settings(tips=TIPS)
        lost = self.paid_tip(100, "Lost")
        self.fake.plan_capture(lost, "lost")
        self.assert_equal(self.viewer.capture_tip(lost), {})
        self.assert_paid(lost, "1.00")
        self.moblin.wait_for_backend_request("message", lost)
        failed = self.paid_tip(700, "Failed")
        self.fake.plan_capture(failed, "failed")
        self.assert_backend_error(502, lambda: self.viewer.capture_tip(failed), self.no_answer())
        self.assert_not_paid(failed)
        self.assert_false(self.received(failed))
        self.assert_equal(self.viewer.capture_tip(failed), {})
        self.assert_paid(failed, "7.00")
        self.moblin.wait_for_backend_request("message", failed)
        ids = [item["id"] for item in self.dashboard.recent("tip")["items"]]
        self.assert_equal([ids.count(lost), ids.count(failed)], [1, 1])


class PaymentDeadlineTestCase(PaymentTestCase):
    """The backend giving viewers 2 seconds to pay instead of half an hour, so a test can be too late."""

    def setup(self):
        self.backend.restart(extra=DEADLINE)
        super().setup()

    def teardown(self):
        try:
            super().teardown()
        finally:
            self.backend.restart()

    def late_tips(self, *users: str) -> list[str]:
        """Tips whose money the provider takes after a review that ends after the deadline."""
        order_ids = [self.paid_tip(100, user) for user in users]
        for order_id in order_ids:
            self.fake.plan_capture(order_id, "pending")
            self.assert_backend_error(402, lambda order_id=order_id: self.viewer.capture_tip(order_id))
        wait_past_deadline()
        for order_id in order_ids:
            self.fake.complete(order_id)
        return order_ids

    def assert_refunded(self, order_id: str):
        if self.provider == "stripe":
            refunds = self.context.stripe.refunds(order_id, self.account_id)
            self.assert_equal([refund["status"] for refund in refunds], ["succeeded"])
            return
        unit = self.context.paypal.order(order_id, self.merchant_id)["purchase_units"][0]
        self.assert_equal(unit["payments"]["captures"][0]["status"], "REFUNDED")


class PaymentsTipTooLate(PaymentDeadlineTestCase):
    """Take no money for a tip the viewer pays after the deadline, and release Stripe's hold on the card."""

    def run(self):
        self.dashboard.save_settings(tips=TIPS)
        too_late = "The tip took too long, so no money was taken. Try again."
        slow = self.paid_tip(100, "Slow")
        unpaid = self.order_tip(100, "Unpaid")
        self.assert_backend_error(402, lambda: self.viewer.capture_tip(unpaid), "The tip was not completed.")
        wait_past_deadline()
        self.assert_backend_error(410, lambda: self.viewer.capture_tip(slow), too_late)
        self.assert_not_paid(slow)
        self.assert_backend_error(410, lambda: self.viewer.capture_tip(unpaid), too_late)
        self.assert_hold_released(unpaid)
        self.assert_backend_error(
            410,
            lambda: self.viewer.capture_tip(unpaid),
            "This tip is unknown or expired, so no money was taken.",
        )


class PaymentsTipRefundedWhenLate(PaymentDeadlineTestCase):
    """Refund a tip whose money is taken after the deadline, and show it late only when the refund is refused."""

    def run(self):
        self.connect_moblin()
        self.dashboard.save_settings(tips=TIPS)
        refunded, kept = self.late_tips("Refunded", "Kept")
        self.fake.refunds = "failed"
        self.assert_backend_error(502, lambda: self.viewer.capture_tip(refunded), self.no_answer())
        self.fake.refunds = "accepted"
        self.assert_backend_error(
            410,
            lambda: self.viewer.capture_tip(refunded),
            "The tip took too long, so the money was refunded.",
        )
        self.assert_refunded(refunded)
        self.assert_false(self.received(refunded))
        self.assert_backend_error(
            410,
            lambda: self.viewer.capture_tip(refunded),
            "This tip is unknown or expired, so no money was taken.",
        )
        self.fake.refunds = "refused"
        self.assert_equal(self.viewer.capture_tip(kept), {})
        self.assert_paid(kept, "1.00")
        tip = self.moblin.wait_for_backend_request("message", kept)
        self.assert_equal(tip.body["name"], "Kept")
        self.assert_true(self.received(kept))
        requests = self.moblin.backend_requests("message")
        self.assert_not_in(refunded, [request.body.get("requestId") for request in requests])


class PaymentsProviderDown(PaymentTestCase):
    """Start no payment while PayPal or Stripe is down, and finish a paid one once it is back."""

    def run(self):
        self.connect_moblin()
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.paid_tip(100, "Down")
        self.fake.down = True
        self.assert_backend_error(
            502,
            lambda: self.order_tip(100),
            f"{self.provider_name} could not start the payment. Try again.",
        )
        self.assert_backend_error(502, lambda: self.viewer.capture_tip(order_id), self.no_answer())
        self.fake.down = False
        self.assert_equal(self.viewer.capture_tip(order_id), {})
        self.assert_paid(order_id, "1.00")
        self.moblin.wait_for_backend_request("message", order_id)


class PaymentsSettledWithoutTheViewer(TestCase):
    """Refund a late payment and give an abandoned one up without the viewer asking again."""

    def run(self):
        self.backend.restart(extra=DEADLINE + ["--settle-interval", "1"])
        abandoned = {}
        late = {}
        for provider in ["paypal", "stripe"]:
            dashboard = self.sign_in(f"settled{provider}")
            if provider == "stripe":
                settings = self.context.page_with_stripe(dashboard)
                seller = settings["stripeAccountId"]
                fake: FakePayPal | FakeStripe = self.backend.fake_stripe
                pay: Callable[[str, str], None] = self.context.stripe.pay_with_card
            else:
                settings = self.context.page_with_paypal(dashboard)
                seller = settings["merchantId"]
                fake = self.backend.fake_paypal
                pay = self.context.paypal.pay_with_card
            dashboard.change_settings(tips=TIPS)
            viewer = self.frontend.viewer(settings["id"])
            abandoned[provider] = (viewer, viewer.order_tip(100, provider=provider), seller)
            late[provider] = (dashboard, viewer.order_tip(100, provider=provider), seller)
            for _, order_id, _ in [abandoned[provider], late[provider]]:
                pay(order_id, seller)
            order_id = late[provider][1]
            fake.plan_capture(order_id, "pending")
            self.assert_backend_error(
                402, lambda viewer=viewer, order_id=order_id: viewer.capture_tip(order_id)
            )
        wait_past_deadline()
        self.backend.fake_paypal.complete(late["paypal"][1])
        self.backend.fake_stripe.complete(late["stripe"][1])

        def refunded() -> bool:
            unit = self.context.paypal.order(late["paypal"][1], late["paypal"][2])["purchase_units"][0]
            return (
                unit["payments"]["captures"][0]["status"] == "REFUNDED"
                and len(self.context.stripe.refunds(late["stripe"][1], late["stripe"][2])) == 1
            )

        def cancelled() -> bool:
            _, order_id, account_id = abandoned["stripe"]
            return self.context.stripe.payment(order_id, account_id)["status"] == "canceled"

        self.wait_until(refunded, "the late payments to be refunded")
        self.wait_until(cancelled, "the abandoned Stripe payment to be cancelled")
        for viewer, order_id, _ in abandoned.values():
            self.assert_backend_error(
                410,
                lambda viewer=viewer, order_id=order_id: viewer.capture_tip(order_id),
                "This tip is unknown or expired, so no money was taken.",
            )
        _, order_id, merchant_id = abandoned["paypal"]
        self.assert_not_equal(self.context.paypal.order(order_id, merchant_id)["status"], "COMPLETED")
        for dashboard, order_id, _ in late.values():
            self.assert_not_in(order_id, [item["id"] for item in dashboard.recent("tip")["items"]])

    def teardown(self):
        self.backend.fake_paypal.reset()
        self.backend.fake_stripe.reset()
        self.backend.restart()


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
            PaymentsTipDeclined,
            PaymentsTipUnderReview,
            PaymentsSpinAndEffectUnderReview,
            PaymentsTipAnswerLost,
            PaymentsTipTooLate,
            PaymentsTipRefundedWhenLate,
            PaymentsProviderDown,
        ]
    ] + [PaymentsSettledWithoutTheViewer(context)]
