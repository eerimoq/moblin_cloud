import threading
import time

from ..utils.context import Context
from ..utils.devices import RawDevice
from ..utils.fake_twitch import user_id_of
from ..utils.frontend import BackendError
from ..utils.test_case import TestCase


class ReceivedTestCase(TestCase):
    """A streamer with PayPal connected and tips on, whose viewers' tips are paid with the fake PayPal."""

    login = "receivedbase"

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.dashboard = self.sign_in(self.login)
        self.settings = self.context.page_with_paypal(self.dashboard)
        self.merchant_id = self.settings["merchantId"]
        self.page_id = self.settings["id"]
        self.viewer = self.frontend.viewer(self.page_id)
        self.dashboard.change_settings(tips={"amounts": [100]})

    def paid_tip(self, amount: int = 100, user: str | None = None, message: str | None = None) -> str:
        order_id = self.viewer.order_tip(amount, user, message)
        self.context.paypal.pay_with_card(order_id, self.merchant_id)
        self.viewer.capture_tip(order_id)
        return order_id

    def recent(self, query: str = "") -> dict:
        return self.dashboard.get(f"/account/recent?kind=tip{query}")


class ReceivedPages(ReceivedTestCase):
    """Page through the received tips newest first, 20 at a time by default and at most 50, without
    losing or repeating any of those received in the same second."""

    login = "receivedpages"

    def run(self):
        order_ids = [self.paid_tip(100 + number, f"Viewer {number}") for number in range(23)]
        newest_first = list(reversed(order_ids))
        first = self.recent()
        self.assert_equal([item["id"] for item in first["items"]], newest_first[:20])
        self.assert_true(first["more"])
        rest = self.recent(f"&before={first['items'][-1]['id']}")
        self.assert_equal([item["id"] for item in rest["items"]], newest_first[20:])
        self.assert_false(rest["more"])
        self.assert_equal([item["id"] for item in self.recent("&limit=50")["items"]], newest_first)
        self.assert_false(self.recent("&limit=23")["more"])
        self.assert_true(self.recent("&limit=22")["more"])
        self.assert_equal(len(self.recent("&limit=1000")["items"]), 23)
        seen: list[str] = []
        query = "&limit=4"
        while True:
            page = self.recent(query)
            seen += [item["id"] for item in page["items"]]
            if not page["more"]:
                break
            query = f"&limit=4&before={page['items'][-1]['id']}"
        self.assert_equal(seen, newest_first, "every tip once, in order, page by page")
        for limit in ["0", "-1", "x"]:
            try:
                self.assert_true(len(self.recent(f"&limit={limit}")["items"]) <= 50)
            except BackendError as error:
                self.assert_true(400 <= error.status < 500, f"limit {limit}: {error.status}")
        try:
            self.recent("&before=NOSUCHORDER")
        except BackendError as error:
            self.assert_true(400 <= error.status < 500, f"unknown before: {error.status}")
        item = first["items"][0]
        self.assert_equal(
            {key: item[key] for key in ["id", "user", "amount", "currency", "provider", "status"]},
            {
                "id": order_ids[-1],
                "user": "Viewer 22",
                "amount": 122,
                "currency": "USD",
                "provider": "paypal",
                "status": "waiting",
            },
        )
        self.assert_equal(item["devices"], [])


class ReceivedByAssistants(ReceivedTestCase):
    """Show assistants the same received items without how they were paid, and let them send one again."""

    login = "receivedmods"

    def run(self):
        order_id = self.paid_tip(250, "Modview", "Hello")
        assistant = self.sign_in("receivedmod")
        self.assert_backend_error(403, lambda: assistant.assisted_resend(self.page_id, order_id))
        self.dashboard.add_assistant("twitch", "receivedmod")
        own = self.recent()["items"]
        assisted = assistant.assisted_recent(self.page_id, "tip")["items"]
        self.assert_equal([item["id"] for item in assisted], [order_id])
        self.assert_equal(assisted, [{key: value for key, value in own[0].items() if key != "provider"}])
        device = RawDevice(self.backend.port, self.dashboard.moblin_cloud_key(), "RM1")
        try:
            self.wait_until(
                lambda: self.recent()["items"][0]["status"] == "delivered", "the tip to be delivered"
            )
            self.assert_equal(assistant.assisted_resend(self.page_id, order_id), {})
            self.wait_until(
                lambda: any(tip["requestId"] == f"{order_id}-1" for tip in device.tips()),
                "the tip sent again",
            )
            self.assert_backend_error(404, lambda: assistant.assisted_resend(self.page_id, "NOSUCHORDER"))
            other = self.sign_in("receivedother")
            other_order = other.page_id()
            self.assert_refused(lambda: assistant.assisted_resend(other_order, order_id))
        finally:
            device.close()
            self.dashboard.remove_assistant("twitch", user_id_of("receivedmod"))


class ReceivedCapturedOnce(ReceivedTestCase):
    """Deliver and count a tip once when the page captures it many times at once."""

    login = "receivedonce"

    def run(self):
        admin = self.sign_in("admin")

        def payments() -> int:
            return next(
                (
                    counter["total"]
                    for counter in admin.admin_stats()["counters"]
                    if counter["name"] == "tipPayments"
                ),
                0,
            )

        before = payments()
        order_id = self.viewer.order_tip(500, "Hasty")
        self.context.paypal.pay_with_card(order_id, self.merchant_id)
        results: list[object] = []

        def capture():
            try:
                results.append(self.viewer.capture_tip(order_id))
            except BackendError as error:
                results.append(error.status)

        threads = [threading.Thread(target=capture) for _ in range(8)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        self.assert_equal(results, 8 * [{}], "every capture answers that the tip went through")
        self.assert_equal([item["id"] for item in self.recent()["items"]], [order_id])
        self.assert_equal(payments() - before, 1)
        device = RawDevice(self.backend.port, self.dashboard.moblin_cloud_key(), "CO1")
        try:
            self.wait_until(lambda: bool(device.tips()), "the tip")
            time.sleep(2)
            self.assert_equal([tip["requestId"] for tip in device.tips()], [order_id])
        finally:
            device.close()


class ReceivedAmounts(ReceivedTestCase):
    """Take any tip from the lowest suggested amount up, charging exactly that much, and refuse nonsense."""

    login = "receivedamounts"

    def run(self):
        for amount in [100, 101, 199, 1000, 123_456_789, 2**32 - 1, 2**53 + 1, 2**63 - 1]:
            try:
                order_id = self.viewer.order_tip(amount)
            except BackendError as error:
                self.assert_true(400 <= error.status < 500, f"{amount}: {error.status} {error.text}")
                continue
            unit = self.context.paypal.order(order_id, self.merchant_id)["purchase_units"][0]
            self.assert_equal(
                unit["amount"]["value"], f"{amount // 100}.{amount % 100:02}", f"tip of {amount}"
            )
        invalid: list = [-100, 2**64, 1.5, "100", None]
        for value in invalid:
            self.assert_refused(lambda value=value: self.viewer.order_tip(value))


class ReceivedTestLimit(TestCase):
    """Refuse more than 10 test sends a minute per page, whoever sends them, and keep only the 5 newest tests."""

    def setup(self):
        self.skip_if_real_twitch_sign_in()

    def run(self):
        dashboard = self.sign_in("testlimit")
        page_id = dashboard.page_id()
        assistant = self.sign_in("testlimitmod")
        dashboard.add_assistant("twitch", "testlimitmod")
        for number in range(1, 8):
            self.assert_equal(dashboard.test_tip(str(number)), {})
        self.assert_equal(assistant.assisted_test_tip(page_id, "8"), {})
        self.assert_equal(dashboard.test_effect(), {})
        self.assert_equal(assistant.assisted_test_effect(page_id), {})
        self.assert_backend_error(429, lambda: dashboard.test_tip("11"))
        self.assert_backend_error(429, lambda: assistant.assisted_test_tip(page_id, "12"))
        self.assert_backend_error(429, dashboard.test_effect)
        other = self.sign_in("testlimitother")
        self.assert_equal(other.test_tip(), {})
        device = RawDevice(self.backend.port, dashboard.moblin_cloud_key(), "TL1")
        try:
            self.wait_until(lambda: len(device.tips()) >= 5, "the test sends")
            time.sleep(3)
            tips = [tip for tip in device.tips() if "message" in tip["kind"]]
            self.assert_equal(
                [tip["message"] for tip in tips], ["4", "5", "6", "7", "8"], "the 5 newest test tips"
            )
            self.assert_equal(len([tip for tip in device.tips() if "effect" in tip["kind"]]), 2)
        finally:
            device.close()


def tests(context: Context):
    return [
        ReceivedPages(context),
        ReceivedByAssistants(context),
        ReceivedCapturedOnce(context),
        ReceivedAmounts(context),
        ReceivedTestLimit(context),
    ]
