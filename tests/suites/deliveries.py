import time

from ..utils.context import Context
from ..utils.devices import RawDevice
from ..utils.events import EventStream
from ..utils.fake_twitch import user_id_of
from ..utils.test_case import TestCase


class DeliveriesTestCase(TestCase):
    """Paid tips of a streamer with PayPal, delivered to the hand made devices of the page."""

    login = "deliveriesbase"

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.dashboard = self.sign_in(self.login)
        self.settings = self.context.page_with_paypal(self.dashboard)
        self.page_id = self.settings["id"]
        self.key = self.dashboard.moblin_cloud_key()
        self.viewer = self.frontend.viewer(self.page_id)
        self.dashboard.change_settings(tips={"amounts": [100]})

    def paid_tip(self, user: str) -> str:
        order_id = self.viewer.order_tip(100, user)
        self.context.paypal.pay_with_card(order_id, self.settings["merchantId"])
        self.viewer.capture_tip(order_id)
        return order_id

    def item(self, order_id: str) -> dict:
        return next(item for item in self.dashboard.recent("tip")["items"] if item["id"] == order_id)

    def device(self, device_id: str) -> RawDevice:
        device = RawDevice(self.backend.port, self.key, device_id)
        self.assert_true(device.ok())
        return device


class DeliveriesWaitForKnownDevices(DeliveriesTestCase):
    """Keep a tip waiting for a known device that is away, until it comes back or the streamer removes it."""

    login = "deliverieswait"

    def run(self):
        away = self.device("AWAY")
        away.close()
        here = self.device("HERE")
        order_id = self.paid_tip("Waiting")
        self.wait_until(lambda: any(tip["requestId"] == order_id for tip in here.tips()), "the tip on HERE")
        self.wait_until(
            lambda: any(device["received"] for device in self.item(order_id)["devices"]), "HERE received"
        )
        item = self.item(order_id)
        self.assert_equal(item["status"], "waiting")
        self.assert_equal(
            sorted((device["name"], device["received"]) for device in item["devices"]),
            [("Moblin 1", False), ("Moblin 2", True)],
        )
        back = self.device("AWAY")
        self.wait_until(lambda: any(tip["requestId"] == order_id for tip in back.tips()), "the tip on AWAY")
        self.wait_until(lambda: self.item(order_id)["status"] == "delivered", "the tip to be delivered")
        back.close()
        second = self.paid_tip("Removed")
        self.wait_until(lambda: any(tip["requestId"] == second for tip in here.tips()), "the second tip")
        time.sleep(1)
        self.assert_equal(self.item(second)["status"], "waiting")
        self.dashboard.remove_device("AWAY")
        self.wait_until(
            lambda: self.item(second)["status"] == "delivered", "the tip to stop waiting for AWAY"
        )
        self.assert_equal([device["name"] for device in self.item(second)["devices"]], ["Moblin 2"])
        here.close()


class DeliveriesRetried(DeliveriesTestCase):
    """Retry a tip Moblin answered with an error until it takes it, and count the failure once."""

    login = "deliveriesretried"

    def run(self):
        admin = self.sign_in("admin")

        def counters() -> dict[str, int]:
            return {counter["name"]: counter["total"] for counter in admin.admin_stats()["counters"]}

        before = counters()
        device = RawDevice(self.backend.port, self.key, "ERR")
        device.answer = lambda data: ({"error": {}}, None) if "tip" in data else ({"ok": {}}, None)
        order_id = self.paid_tip("Retried")
        self.wait_until(lambda: len([r for r in device.requests if "tip" in r]) >= 2, "a retry", timeout=30)
        self.assert_equal(self.item(order_id)["status"], "waiting")
        failed = {
            name: total - before.get(name, 0)
            for name, total in counters().items()
            if name.startswith("tipDeliveriesFailed") and total != before.get(name, 0)
        }
        self.assert_equal(list(failed.values()), [1], f"one failure counted {failed}")
        device.answer = lambda data: ({"ok": {}}, None)
        self.wait_until(
            lambda: self.item(order_id)["status"] == "delivered", "the tip to be delivered", timeout=30
        )
        after = counters()
        self.assert_equal(after.get("tipDeliveries", 0) - before.get("tipDeliveries", 0), 1)
        self.assert_equal(
            sum(
                total - before.get(name, 0)
                for name, total in after.items()
                if name.startswith("tipDeliveriesFailed")
            ),
            1,
        )
        device.close()


class DeliveriesPending(DeliveriesTestCase):
    """Count a tip no device has got as pending for the operator, and not once a device got it."""

    login = "deliveriespending"

    def run(self):
        admin = self.sign_in("admin")

        def pending() -> int:
            return admin.admin_stats()["pending"]["deliveries"]

        before = pending()
        self.paid_tip("Pending")
        self.assert_equal(pending(), before + 1)
        away = self.device("PEND1")
        self.wait_until(lambda: pending() == before, "the tip to be got")
        away.close()


class DeliveriesStreamsRechecked(DeliveriesTestCase):
    """End a moderator's open streams within a minute of being removed, and everyone's when signing out everywhere."""

    login = "deliveriesrecheck"

    def run(self):
        device = self.device("RC1")
        moderator = self.sign_in("recheckmod")
        self.dashboard.add_moderator("twitch", "recheckmod")
        streams = [
            EventStream(moderator, f"/account/pages/{self.page_id}/devices/RC1/events"),
            EventStream(moderator, f"/account/pages/{self.page_id}/events"),
        ]
        own = EventStream(self.dashboard, f"/account/pages/{self.page_id}/devices/RC1/events")
        self.dashboard.remove_moderator("twitch", user_id_of("recheckmod"))
        self.dashboard.sign_out_everywhere()
        for stream in streams + [own]:
            self.assert_true(stream.wait_for_end(75), "the stream ended")
            stream.close()
        device.close()


def tests(context: Context):
    return [
        DeliveriesWaitForKnownDevices(context),
        DeliveriesRetried(context),
        DeliveriesPending(context),
        DeliveriesStreamsRechecked(context),
    ]
