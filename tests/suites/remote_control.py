from ..utils.config import Capability
from ..utils.context import Context
from ..utils.moblin import Moblin
from ..utils.moblin import wheel_option
from ..utils.test_case import TestCase

WHEEL = [wheel_option("Dance"), wheel_option("Spin again", 4)]


class RemoteControlTestCase(TestCase):
    """The streamer's real Moblin connected to the backend with the streamer URL from the dashboard."""

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


class MoblinConnects(RemoteControlTestCase):
    """Show in the dashboard that Moblin is connected, and Moblin's song queue on the page."""

    def run(self):
        self.moblin.disconnect_from_backend()
        self.wait_until(lambda: not self.connected(), "Moblin to be disconnected")
        self.moblin.connect_to_backend(self.dashboard.streamer_url())
        self.wait_until(self.connected, "Moblin to be connected")
        if self.moblin.has_capability(Capability.APPLE_MUSIC):
            queue = self.viewer.queue()
            self.assert_in("playing", queue)
            self.assert_in("songs", queue)
            self.assert_true(any(request.kind == "status" for request in self.moblin.backend_requests()))
        self.moblin.disconnect_from_backend()
        self.wait_until(lambda: not self.connected(), "Moblin to be disconnected")


class MoblinReconnects(RemoteControlTestCase):
    """Show Moblin as connected again after its connection drops."""

    def run(self):
        self.moblin.connect_to_backend(self.dashboard.streamer_url())
        old = self.moblin.disconnect()
        self.moblin.wait_for_connection(relayed=True, other_than=old)
        self.wait_until(self.connected, "Moblin to be connected again")


class MoblinWheel(RemoteControlTestCase):
    """Show the wheel picked in Moblin's remote control settings on the page, and no wheel without one."""

    def run(self):
        self.moblin.connect_to_backend(self.dashboard.streamer_url(), WHEEL)
        expected = {"options": [{"text": option["text"], "weight": option["weight"]} for option in WHEEL]}
        self.wait_until(lambda: self.viewer.spin()["wheel"] == expected, "the wheel to be on the page")
        wheel = self.moblin.wheel()
        assert wheel is not None
        self.assert_equal(wheel["options"][1]["text"], "Spin again")
        self.moblin.connect_to_backend(self.dashboard.streamer_url())
        self.wait_until(lambda: self.viewer.spin()["wheel"] is None, "the wheel to be gone")


class MoblinWheelChanges(RemoteControlTestCase):
    """Follow changes to Moblin's wheel while it stays connected."""

    def run(self):
        if not self.moblin.is_fake():
            self.skip_if_not_interactive()
        self.moblin.connect_to_backend(self.dashboard.streamer_url(), WHEEL)
        expected = {"options": [{"text": option["text"], "weight": option["weight"]} for option in WHEEL]}
        self.wait_until(lambda: self.viewer.spin()["wheel"] == expected, "the wheel to be on the page")
        self.moblin.change_wheel(None)
        self.wait_until(lambda: self.viewer.spin()["wheel"] is None, "the removed wheel to be gone")
        self.moblin.change_wheel(WHEEL)
        self.wait_until(lambda: self.viewer.spin()["wheel"] == expected, "the wheel to be back on the page")


class NewStreamerUrlDisconnectsMoblin(RemoteControlTestCase):
    """Disconnect Moblin when the streamer replaces the streamer URL, until it gets the new one."""

    def run(self):
        self.moblin.connect_to_backend(self.dashboard.streamer_url())
        old_key = self.moblin.connected_key()
        new_url = self.dashboard.new_streamer_url()["streamerUrl"]
        self.assert_false(self.dashboard.account()["settings"]["connected"])
        self.moblin.wait_for_connection(key=old_key, relayed=False)
        self.assert_false(self.connected())
        self.moblin.connect_to_backend(new_url)
        self.wait_until(self.connected, "Moblin to be connected with the new streamer URL")


def tests(context: Context):
    return [
        MoblinConnects(context),
        MoblinReconnects(context),
        MoblinWheel(context),
        MoblinWheelChanges(context),
        NewStreamerUrlDisconnectsMoblin(context),
    ]
