from ..utils.config import Capability
from ..utils.context import Context
from ..utils.moblin import Moblin
from ..utils.moblin import wheel_option
from ..utils.test_case import TestCase

WHEEL = [wheel_option("Dance"), wheel_option("Spin again", 4)]


class RemoteControlTestCase(TestCase):
    """The streamer's Moblin connected to the backend with the Moblin Cloud key from the dashboard."""

    def setup(self):
        self.skip_if_no_moblin()
        self.dashboard = self.sign_in()
        self.dashboard.create_page()
        self.viewer = self.website.viewer(self.dashboard.page_id())

    @property
    def moblin(self) -> Moblin:
        assert self.context.moblin is not None
        return self.context.moblin

    def connected(self) -> bool:
        return self.dashboard.account()["settings"]["connected"]


class RemoteControlConnects(RemoteControlTestCase):
    """Show in the dashboard that Moblin is connected, and Moblin's song queue on the page."""

    def run(self):
        self.moblin.disconnect_from_backend()
        self.wait_until(lambda: not self.connected(), "Moblin to be disconnected")
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key())
        self.wait_until(self.connected, "Moblin to be connected")
        if self.moblin.has_capability(Capability.APPLE_MUSIC):
            queue = self.viewer.queue()
            self.assert_in("playing", queue)
            self.assert_in("songs", queue)
            self.assert_true(any(request.kind == "status" for request in self.moblin.backend_requests()))
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
        new_key = self.dashboard.new_moblin_cloud_key()["devices"][0]["key"]
        self.assert_false(self.dashboard.account()["settings"]["connected"])
        self.moblin.wait_for_rejected_key(old_key, start=position)
        self.assert_false(self.connected())
        self.moblin.connect_to_backend(new_key)
        self.wait_until(self.connected, "Moblin to be connected with the new Moblin Cloud key")


def tests(context: Context):
    return [
        RemoteControlConnects(context),
        RemoteControlReconnects(context),
        RemoteControlWheel(context),
        RemoteControlWheelChanges(context),
        RemoteControlNewMoblinCloudKeyDisconnects(context),
    ]
