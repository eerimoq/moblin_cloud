from ..utils.config import Capability
from ..utils.context import Context
from ..utils.moblin import Moblin
from ..utils.moblin import wheel_option
from ..utils.test_case import TestCase

WHEEL = [wheel_option("Dance"), wheel_option("Spin again", 4)]
SONG = "https://music.apple.com/us/album/take-on-me/380907762?i=380907765"
SONG_LINK = "https://music.apple.com/song/380907765?i=380907765"
MISSING_SONG = "https://music.apple.com/us/song/does-not-exist/99999999999"


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


class RemoteControlTestDeliveries(RemoteControlTestCase):
    """Send a free test tip, wheel spin and song from the dashboard, which Moblin gets like paid ones."""

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
        if not self.moblin.has_capability(Capability.APPLE_MUSIC):
            return
        self.moblin.clear_music()
        self.assert_backend_error(
            400, lambda: self.dashboard.test_song("Take on me"), "Paste the Apple Music share link of a song."
        )
        self.assert_backend_error(
            422,
            lambda: self.dashboard.test_song(MISSING_SONG),
            "That song is not available on the streamer's Apple Music.",
        )
        start = self.moblin.backend_requests_position()
        self.assert_equal(self.dashboard.test_song(SONG), {})
        add = self.moblin.wait_for_new_backend_request("add", start)
        self.assert_equal([add.body["title"], add.body["user"]], [SONG_LINK, name])
        self.wait_until(
            lambda: [song.get("user") for song in self.viewer.queue()["songs"]] == [name],
            "the test song to be in the queue on the page",
        )

    def teardown(self):
        if self.context.moblin is not None and self.moblin.has_capability(Capability.APPLE_MUSIC):
            self.moblin.clear_music()


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


class RemoteControlSongRequestsOff(RemoteControlTestCase):
    """Refuse songs while song requests are off in Moblin, but keep showing the queue."""

    def run(self):
        self.skip_if_missing_capability(Capability.APPLE_MUSIC)
        if not self.moblin.is_fake():
            self.skip_if_not_interactive()
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key())
        self.wait_until(lambda: self.viewer.queue()["songRequests"], "song requests to be on")
        self.moblin.change_song_requests(False)
        self.wait_until(lambda: not self.viewer.queue()["songRequests"], "song requests to be off")
        self.assert_in("songs", self.viewer.queue())
        self.assert_backend_error(
            409,
            lambda: self.dashboard.test_song(SONG),
            "The streamer is not taking song requests right now.",
        )
        self.moblin.change_song_requests(True)
        self.wait_until(lambda: self.viewer.queue()["songRequests"], "song requests to be on again")

    def teardown(self):
        if self.context.moblin is not None and self.moblin.is_fake():
            self.moblin.change_song_requests(True)


class RemoteControlChosenDevices(RemoteControlTestCase):
    """Send songs and spins to the devices picked in the dashboard, tips to any connected device, and nothing to a device turned off."""

    login = "devices"

    def run(self):
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key(), WHEEL)
        self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "the wheel to be on the page")
        first = self.dashboard.device_id()
        other = self.dashboard.add_device("Other phone")["devices"][-1]["id"]
        self.dashboard.choose_devices(other, other)
        spin = self.viewer.spin()
        self.assert_equal([spin["wheel"], spin["connected"]], [None, False])
        self.assert_true(self.connected())
        self.assert_backend_error(409, lambda: self.dashboard.test_song(SONG), "Connect Moblin first.")
        self.assert_backend_error(409, self.dashboard.test_spin, "Connect Moblin first.")
        start = self.moblin.backend_requests_position()
        self.dashboard.test_tip()
        self.moblin.wait_for_new_backend_request("tip", start)
        self.dashboard.choose_devices(first, first)
        self.assert_true(self.viewer.spin()["connected"])
        self.dashboard.enable_device(first, False)
        self.assert_false(self.viewer.spin()["connected"])
        self.assert_backend_error(409, self.dashboard.test_spin, "Connect Moblin first.")
        start = self.moblin.backend_requests_position()
        self.dashboard.test_tip()
        self.dashboard.enable_device(first, True)
        self.assert_true(self.viewer.spin()["connected"])
        self.moblin.wait_for_new_backend_request("tip", start)
        self.dashboard.remove_device(other)
        self.assert_equal(self.dashboard.account()["musicDevice"], first)

    def teardown(self):
        if not hasattr(self, "dashboard"):
            return
        devices = self.dashboard.account()["devices"]
        if not devices[0]["enabled"]:
            self.dashboard.enable_device(devices[0]["id"], True)
        for device in devices[1:]:
            self.dashboard.remove_device(device["id"])


def tests(context: Context):
    return [
        RemoteControlConnects(context),
        RemoteControlReconnects(context),
        RemoteControlWheel(context),
        RemoteControlWheelChanges(context),
        RemoteControlNewMoblinCloudKeyDisconnects(context),
        RemoteControlTestDeliveries(context),
        RemoteControlNoWheel(context),
        RemoteControlSongRequestsOff(context),
        RemoteControlChosenDevices(context),
    ]
