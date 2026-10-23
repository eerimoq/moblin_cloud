from ..utils.context import Context
from ..utils.fake_moblin import FakeMoblin
from ..utils.moblin import Moblin
from ..utils.moblin import wheel_option
from ..utils.test_case import TestCase

WHEEL = [wheel_option("Dance"), wheel_option("Spin again", 4)]


class RemoteControlTestCase(TestCase):
    """The streamer's Moblin connected to the backend with the Moblin Cloud key from the dashboard."""

    login = "streamer"

    def setup(self):
        self.skip_if_no_moblin()
        self.dashboard = self.sign_in(self.login)
        self.viewer = self.website.viewer(self.dashboard.page_id())

    @property
    def moblin(self) -> Moblin:
        assert self.context.moblin is not None
        return self.context.moblin

    def connected(self) -> bool:
        return self.dashboard.account()["settings"]["connected"]


class RemoteControlConnects(RemoteControlTestCase):
    """Show in the dashboard that Moblin is connected."""

    def run(self):
        self.moblin.disconnect_from_backend()
        self.wait_until(lambda: not self.connected(), "Moblin to be disconnected")
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key())
        self.wait_until(self.connected, "Moblin to be connected")
        self.moblin.disconnect_from_backend()
        self.wait_until(lambda: not self.connected(), "Moblin to be disconnected")


class RemoteControlReconnects(RemoteControlTestCase):
    """Show Moblin as connected again after its connection drops."""

    def run(self):
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key())
        old = self.moblin.disconnect()
        self.moblin.wait_for_backend_connection(self.dashboard.moblin_cloud_key(), other_than=old)
        self.wait_until(self.connected, "Moblin to be connected again")


class RemoteControlWheel(RemoteControlTestCase):
    """Show the wheel picked in Moblin's Moblin Cloud settings on the page, and no wheel without one."""

    def run(self):
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key(), WHEEL)
        expected = {"options": [{"text": option["text"], "weight": option["weight"]} for option in WHEEL]}
        self.wait_until(lambda: self.viewer.spin()["wheel"] == expected, "the wheel to be on the page")
        wheel = self.moblin.wheel()
        assert wheel is not None
        self.assert_equal(wheel["options"][1]["text"], "Spin again")
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key())
        self.wait_until(lambda: self.viewer.spin()["wheel"] is None, "the wheel to be gone")


class RemoteControlWheelChanges(RemoteControlTestCase):
    """Follow changes to Moblin's wheel while it stays connected."""

    def run(self):
        if not self.moblin.is_fake():
            self.skip_if_not_interactive()
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key(), WHEEL)
        expected = {"options": [{"text": option["text"], "weight": option["weight"]} for option in WHEEL]}
        self.wait_until(lambda: self.viewer.spin()["wheel"] == expected, "the wheel to be on the page")
        self.moblin.change_wheel(None)
        self.wait_until(lambda: self.viewer.spin()["wheel"] is None, "the removed wheel to be gone")
        self.moblin.change_wheel(WHEEL)
        self.wait_until(lambda: self.viewer.spin()["wheel"] == expected, "the wheel to be back on the page")


class RemoteControlNewMoblinCloudKeyDisconnects(RemoteControlTestCase):
    """Disconnect Moblin when the streamer replaces the Moblin Cloud key, until it gets the new one."""

    def run(self):
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key())
        old_key = self.moblin.connected_key()
        assert old_key is not None
        position = self.moblin.rejected_keys_position()
        new_key = self.dashboard.new_moblin_cloud_key()["key"]
        self.assert_false(self.dashboard.account()["settings"]["connected"])
        self.moblin.wait_for_rejected_key(old_key, start=position)
        self.assert_false(self.connected())
        self.moblin.connect_to_backend(new_key)
        self.wait_until(self.connected, "Moblin to be connected with the new Moblin Cloud key")


class RemoteControlTestDeliveries(RemoteControlTestCase):
    """Send a free test tip and wheel spin from the dashboard, which Moblin gets like paid ones."""

    def run(self):
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key(), WHEEL)
        self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "the wheel to be on the page")
        account = self.dashboard.account()
        name = account["name"]
        tips = account["settings"]["tips"]
        amount = tips["amounts"][0] if tips is not None and tips["enabled"] else 500
        start = self.moblin.backend_requests_position()
        self.assert_equal(self.dashboard.test_tip("Testing tips"), {})
        tip = self.moblin.wait_for_new_backend_request("tip", start)
        self.assert_equal(
            {key: value for key, value in tip.body.items() if key != "requestId"},
            {
                "name": name,
                "message": "Testing tips",
                "amount": f"{amount // 100}.{amount % 100:02}",
                "currency": account["settings"]["currency"],
            },
        )
        spin = account["settings"]["spin"]
        spins = spin["options"][0]["spins"] if spin is not None and spin["enabled"] else 1
        start = self.moblin.backend_requests_position()
        self.assert_equal(self.dashboard.test_spin(), {})
        request = self.moblin.wait_for_new_backend_request("spinWheelOfLuck", start)
        self.assert_equal([request.body["name"], request.body["count"]], [name, spins])
        self.assert_not_equal(request.body["requestId"], tip.body["requestId"])


class RemoteControlNoWheel(RemoteControlTestCase):
    """Tell the streamer to pick a wheel in Moblin when testing a spin while Moblin has none."""

    def run(self):
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key())
        self.wait_until(lambda: self.viewer.spin()["connected"], "Moblin to be connected")
        self.assert_equal(self.viewer.spin()["wheel"], None)
        self.assert_backend_error(
            409,
            self.dashboard.test_spin,
            "Pick a wheel of luck in Moblin's Moblin Cloud settings first.",
        )


class RemoteControlDevices(RemoteControlTestCase):
    """Send spins to the first device with a wheel and tips to every device, all connected with the one Moblin Cloud key."""

    login = "devices"

    def connected(self):
        return [device for device in self.dashboard.account()["devices"] if device["connected"]]

    def run(self):
        key = self.dashboard.moblin_cloud_key()
        self.moblin.connect_to_backend(key, WHEEL)
        self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "the wheel to be on the page")
        self.second = FakeMoblin(None, self.backend.url, key=key, device_id="SECOND")
        self.second.start()
        self.wait_until(lambda: len(self.connected()) == 2, "the second device to connect")
        devices = self.connected()
        self.assert_equal([(device["id"], device["wheel"]) for device in devices][1], ("SECOND", False))
        self.assert_true(devices[0]["wheel"])
        start = self.moblin.backend_requests_position()
        self.dashboard.test_tip()
        self.moblin.wait_for_new_backend_request("tip", start)
        self.wait_until(lambda: len(self.second.handled_request_ids) == 1, "the second device to get the tip")
        self.second.stop()
        self.wait_until(lambda: len(self.connected()) == 1, "the second device to disconnect")

    def teardown(self):
        if hasattr(self, "second"):
            self.second.stop()


def tests(context: Context):
    return [
        RemoteControlConnects(context),
        RemoteControlReconnects(context),
        RemoteControlWheel(context),
        RemoteControlWheelChanges(context),
        RemoteControlNewMoblinCloudKeyDisconnects(context),
        RemoteControlTestDeliveries(context),
        RemoteControlNoWheel(context),
        RemoteControlDevices(context),
    ]
