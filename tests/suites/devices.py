import json
import time

from websockets.exceptions import ConnectionClosed

from ..utils.context import Context
from ..utils.devices import RawDevice
from ..utils.events import EventStream
from ..utils.test_case import TestCase


class DevicesTestCase(TestCase):
    """Devices of one streamer's page speaking Moblin Cloud's WebSocket by hand, each test with its own streamer."""

    login = "devicesbase"

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.dashboard = self.sign_in(self.login)
        self.page_id = self.dashboard.page_id()
        self.key = self.dashboard.moblin_cloud_key()
        self.viewer = self.frontend.viewer(self.page_id)
        self.raw: list[RawDevice] = []

    def teardown(self):
        for device in getattr(self, "raw", []):
            try:
                device.close()
            except ConnectionClosed:
                pass

    def connect(self, device_id: str | None, identify: dict | None = None) -> RawDevice:
        device = RawDevice(self.backend.port, self.key, device_id, identify)
        self.raw.append(device)
        return device

    def devices(self) -> list[dict]:
        return self.dashboard.account()["devices"]

    def find_device(self, device_id: str) -> dict | None:
        return next((device for device in self.devices() if device["id"] == device_id), None)

    def device(self, device_id: str) -> dict:
        device = self.find_device(device_id)
        assert device is not None, f"device {device_id} is listed"
        return device

    def wait_for_device(self, device_id: str, **fields):
        def check() -> bool:
            device = self.find_device(device_id)
            return device is not None and all(device[name] == value for name, value in fields.items())

        self.wait_until(check, f"device {device_id} to be {fields}", timeout=15)


class DevicesNeedAnId(DevicesTestCase):
    """Refuse a device that does not say which device it is, with an identified error, and do not list it."""

    login = "devicesnoid"

    def run(self):
        for identify in [None, {"authentication": "x", "streamerId": ""}]:
            device = self.connect(None, identify)
            self.assert_true(device.identified is not None, "the backend answers the identify")
            assert device.identified is not None
            self.assert_in("error", device.identified["identified"]["result"])
            self.assert_true(device.closed.wait(10), "the backend closes the refused device")
        self.assert_equal(self.devices(), [])
        self.assert_false(self.dashboard.account()["settings"]["connected"])


class DevicesNamed(DevicesTestCase):
    """Name each new device Moblin with the smallest number no other device of the page is named with."""

    login = "devicesnamed"

    def run(self):
        for number in [1, 2, 3]:
            self.assert_true(self.connect(f"N{number}").ok())
        self.assert_equal([device["name"] for device in self.devices()], ["Moblin 1", "Moblin 2", "Moblin 3"])
        self.assert_equal(self.dashboard.rename_device("N1", "Moblin 4")["devices"][0]["name"], "Moblin 4")
        self.assert_true(self.connect("N4").ok())
        self.assert_equal(self.device("N4")["name"], "Moblin 1")
        self.assert_true(self.connect("N5").ok())
        self.assert_equal(self.device("N5")["name"], "Moblin 5")
        self.assert_equal(self.dashboard.rename_device("N2", " Spare ")["devices"][1]["name"], "Spare")
        self.assert_equal(self.dashboard.rename_device("N3", 30 * "å")["devices"][2]["name"], 30 * "å")
        self.assert_backend_error(
            400, lambda: self.dashboard.rename_device("N3", 31 * "x"), "the name must be 1 to 30 characters"
        )
        self.assert_backend_error(
            400, lambda: self.dashboard.rename_device("N3", ""), "the name must be 1 to 30 characters"
        )
        self.raw[1].close()
        self.wait_for_device("N2", connected=False)
        self.dashboard.remove_device("N2")
        self.assert_equal(self.find_device("N2"), None)
        self.raw[3].close()
        self.wait_for_device("N4", connected=False)
        self.assert_true(self.connect("N6").ok())
        self.assert_equal(self.device("N6")["name"], "Moblin 2")
        self.assert_true(self.connect("N2").ok())
        self.assert_equal(self.device("N2")["name"], "Moblin 3", "a removed device is new when it comes back")
        self.assert_equal(self.device("N4")["name"], "Moblin 1", "a disconnected device keeps its name")
        self.assert_backend_error(404, lambda: self.dashboard.remove_device("NOPE"))
        names = [device["name"] for device in self.devices()]
        self.assert_equal(len(names), len(set(names)), f"unique names {names}")


class DevicesAtMostFive(DevicesTestCase):
    """Refuse a sixth device, but let a connected one connect again in its place."""

    login = "devicesfive"

    def run(self):
        for number in range(1, 6):
            self.assert_true(self.connect(f"F{number}").ok())
        sixth = self.connect("F6")
        self.assert_false(sixth.ok())
        assert sixth.identified is not None
        self.assert_in("error", sixth.identified["identified"]["result"])
        self.assert_true(sixth.closed.wait(10))
        self.assert_equal(self.find_device("F6"), None, "the refused device is not listed")
        old = self.raw[2]
        again = self.connect("F3")
        self.assert_true(again.ok(), "a connected device connecting again replaces its session")
        self.assert_true(old.closed.wait(10), "the replaced connection is closed")
        time.sleep(1)
        devices = self.devices()
        self.assert_equal([device["id"] for device in devices], [f"F{number}" for number in range(1, 6)])
        self.assert_true(all(device["connected"] for device in devices), devices)
        again.send({"ping": {}})
        again.wait_for(lambda message: "pong" in message)
        self.raw[0].close()
        self.wait_for_device("F1", connected=False)
        self.assert_true(self.connect("F7").ok(), "a device may connect once another left")


class DevicesReconnecting(DevicesTestCase):
    """Keep a dropped device's session for a minute, with its place, wheel and tips waiting for it."""

    login = "devicesreconnect"

    def run(self):
        first = self.connect("R1")
        self.assert_true(first.ok())
        first.push_wheel(["Dance", "Sing"])
        self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "the wheel to be on the page")
        self.assert_true(self.connect("R2").ok())
        first.close(1001)
        self.wait_for_device("R1", reconnecting=True)
        self.assert_equal([device["id"] for device in self.devices()], ["R1", "R2"])
        self.assert_true(self.dashboard.account()["settings"]["connected"])
        self.assert_equal(
            self.viewer.spin()["wheel"],
            {"options": [{"text": "Dance", "weight": 1}, {"text": "Sing", "weight": 1}]},
        )
        self.assert_true(self.device("R1")["wheel"])
        self.dashboard.test_tip("While away")
        time.sleep(2)
        tip = self.dashboard.account()
        self.assert_true(tip["settings"]["connected"])
        back = self.connect("R1")
        self.assert_true(back.ok())
        self.wait_for_device("R1", reconnecting=False, connected=True)
        self.assert_equal([device["id"] for device in self.devices()], ["R1", "R2"])
        self.wait_until(
            lambda: any(t["message"] == "While away" for t in back.tips()), "the waiting tip on reconnect"
        )
        self.assert_true(self.device("R1")["wheel"])
        back.close(1000)
        self.wait_for_device("R1", connected=False, reconnecting=False)
        self.assert_true(self.device("R1")["lastSeen"] is not None)
        self.assert_false(self.device("R1")["wheel"], "a device that left is not the wheel device")
        self.assert_equal(self.viewer.spin()["wheel"], None, "R2 has no wheel")
        self.assert_true(self.viewer.spin()["connected"])


class DevicesReconnectingCount(DevicesTestCase):
    """Count a reconnecting device towards the five devices of a page."""

    login = "devicesreconnectcount"

    def run(self):
        for number in range(1, 6):
            self.assert_true(self.connect(f"C{number}").ok())
        self.raw[0].close(1001)
        self.wait_for_device("C1", reconnecting=True)
        self.assert_false(self.connect("C6").ok())
        self.assert_true(self.connect("C1").ok())


class DevicesCloseHandshake(DevicesTestCase):
    """Answer Moblin's close frame at once whatever its code, as RFC 6455 asks, so neither side waits for the other."""

    login = "devicesclose"

    def run(self):
        for code in [1000, 1001, 4000]:
            device = self.connect("H1")
            self.assert_true(device.ok())
            started = time.monotonic()
            device.websocket.close(code)
            took = time.monotonic() - started
            self.assert_true(took < 2, f"closing with {code} took {took:.1f} seconds")
            self.wait_for_device("H1", connected=False)


class DevicesAtMostTwenty(DevicesTestCase):
    """Keep at most 20 devices per account, forgetting the one seen longest ago."""

    login = "devicestwenty"

    def run(self):
        for number in range(1, 22):
            device = self.connect(f"T{number:02}")
            self.assert_true(device.ok())
            device.close()
            self.wait_for_device(f"T{number:02}", connected=False)
            time.sleep(1.1)
        devices = self.devices()
        self.assert_equal(
            [device["id"] for device in devices], [f"T{number:02}" for number in range(21, 1, -1)]
        )
        names = [device["name"] for device in devices]
        self.assert_equal(len(names), len(set(names)), f"unique names {names}")


class DevicesProtocol(DevicesTestCase):
    """Answer pings, and keep the connection when Moblin sends something the backend does not understand."""

    login = "devicesprotocol"

    def run(self):
        device = self.connect("P1")
        self.assert_true(device.ok())
        self.assert_equal(sorted(device.hello["hello"]), ["apiVersion", "authentication"])
        device.send({"ping": {}})
        self.assert_equal(device.wait_for(lambda message: "pong" in message), {"pong": {}})
        for garbage in [
            "not json",
            "{}",
            "[]",
            json.dumps({"event": {"data": {"unknown": {}}}}),
            json.dumps({"response": {"id": 12345, "result": {"ok": {}}}}),
            json.dumps({"identify": {"streamerId": "OTHER", "authentication": "x"}}),
            json.dumps({"event": {"data": {"icon": {"id": "NoSuchIcon"}}}}),
            json.dumps({"event": {"data": {"wheelOfLuck": {"wheel": {"options": "x"}}}}}),
        ]:
            device.send(garbage)
        device.send({"ping": {}})
        device.wait_for(lambda message: "pong" in message)
        self.assert_false(device.closed.is_set())
        self.assert_equal([device["id"] for device in self.devices()], ["P1"])
        self.assert_equal(self.viewer.profile()["icon"], None)
        device.event({"icon": {"id": "AppIconKing"}})
        self.wait_until(lambda: self.viewer.profile()["icon"] == "king", "the icon")
        device.event({"icon": {"id": "NoSuchIcon"}})
        time.sleep(1)
        self.assert_equal(self.viewer.profile()["icon"], "king", "an unknown icon is ignored")
        self.dashboard.test_tip("Still works")
        self.wait_until(lambda: any(tip["message"] == "Still works" for tip in device.tips()), "the tip")


class DevicesWheelDevice(DevicesTestCase):
    """Spin the wheel on the first device with a wheel, and send tips to every device."""

    login = "deviceswheel"

    def run(self):
        first = self.connect("W1")
        second = self.connect("W2")
        second.push_wheel(["Second"])
        self.wait_for_device("W2", wheel=True)
        self.assert_false(self.device("W1")["wheel"])
        self.assert_equal(self.viewer.spin()["wheel"]["options"][0]["text"], "Second")
        self.dashboard.test_spin("To second")
        self.wait_until(lambda: any(t["message"] == "To second" for t in second.tips()), "the spin on W2")
        first.push_wheel(["First"])
        self.wait_for_device("W1", wheel=True)
        self.assert_false(self.device("W2")["wheel"])
        self.assert_equal(self.viewer.spin()["wheel"]["options"][0]["text"], "First")
        self.dashboard.test_spin("To first")
        self.wait_until(lambda: any(t["message"] == "To first" for t in first.tips()), "the spin on W1")
        self.dashboard.test_tip("To both")
        self.wait_until(
            lambda: all(any(t["message"] == "To both" for t in device.tips()) for device in [first, second]),
            "the tip on both devices",
        )
        time.sleep(2)
        self.assert_false(
            any(t["message"] == "To first" for t in second.tips()), "the spin went to one device"
        )
        self.assert_equal(sum(t["message"] == "To both" for t in first.tips()), 1)
        first.push_wheel(None)
        self.wait_for_device("W2", wheel=True)
        self.assert_equal(self.viewer.spin()["wheel"]["options"][0]["text"], "Second")


class DevicesControls(DevicesTestCase):
    """Send a control to a device, with clear answers when it is offline, unknown, slow or fails."""

    login = "devicescontrols"

    def run(self):
        device = self.connect("K1")
        mute = {"setMute": {"on": True}}
        self.assert_equal(self.dashboard.control_device(self.page_id, "K1", mute), {})
        self.wait_until(lambda: mute in device.requests, "the control to reach the device")
        message = {"sendMessage": {"text": "Drink some water!", "name": "Someone else"}}
        self.assert_equal(self.dashboard.control_device(self.page_id, "K1", message), {})
        signed = {"sendMessage": {"text": "Drink some water!", "name": self.dashboard.account()["name"]}}
        self.wait_until(lambda: signed in device.requests, "the message to reach the device under our name")
        for invalid in [
            {},
            {"setMute": {}},
            {"sendMessage": {"text": " "}},
            {"sendMessage": {"text": "x" * 301}},
            {"setScene": {"id": "not-a-uuid"}},
            {"importSettings": {"data": ""}},
            {"setMute": {"on": True}, "setTorch": {"on": True}},
        ]:
            self.assert_backend_error(
                422, lambda invalid=invalid: self.dashboard.control_device(self.page_id, "K1", invalid)
            )
        self.assert_equal(
            [request for request in device.requests if "importSettings" in request], [], "never sent on"
        )
        device.answer = lambda data: None
        started = time.monotonic()
        self.assert_backend_error(
            504,
            lambda: self.dashboard.control_device(self.page_id, "K1", mute),
            "Moblin did not answer. Try again.",
        )
        self.assert_true(4 <= time.monotonic() - started < 15, "waited about 5 seconds")
        device.answer = lambda data: ({"error": {}}, None)
        self.assert_backend_error(
            502,
            lambda: self.dashboard.control_device(self.page_id, "K1", mute),
            "Moblin could not answer. Try again.",
        )
        device.answer = lambda data: ({"ok": {}}, None)
        device.close()
        self.wait_for_device("K1", connected=False)
        self.assert_backend_error(409, lambda: self.dashboard.control_device(self.page_id, "K1", mute))
        self.assert_backend_error(404, lambda: self.dashboard.control_device(self.page_id, "NOPE", mute))
        outsider = self.sign_in("devicesoutsider")
        for control in [mute, {"bogus": {}}]:
            self.assert_backend_error(
                403, lambda control=control: outsider.control_device(self.page_id, "K1", control)
            )
        other_page = outsider.page_id()
        self.assert_refused(lambda: self.dashboard.control_device(other_page, "K1", mute))


class DevicesEvents(DevicesTestCase):
    """Stream a device's state to its device page, also while it is not connected."""

    login = "devicesevents"

    def run(self):
        self.assert_backend_error(
            404,
            lambda: EventStream(self.dashboard, f"/account/pages/{self.page_id}/devices/NOPE/events"),
            "This Moblin is not in the list.",
        )
        device = self.connect("E1")
        device.close()
        self.wait_for_device("E1", connected=False)
        path = f"/account/pages/{self.page_id}/devices/E1/events"
        outsider = self.sign_in("devicesoutsider")
        self.assert_backend_error(403, lambda: EventStream(outsider, path))
        with EventStream(self.dashboard, path) as events:
            name, snapshot = events.next()
            self.assert_equal(name, "snapshot")
            assert isinstance(snapshot, dict)
            self.assert_equal([snapshot["name"], snapshot["connected"]], ["Moblin 1", False])
            device = self.connect("E1")
            self.assert_equal(events.wait_for("connection"), {"connected": True})
            self.wait_until(
                lambda: any("startStatus" in request for request in device.requests), "startStatus"
            )
            self.wait_until(
                lambda: any("getSettings" in request for request in device.requests), "getSettings"
            )
            device.event({"state": {"data": {"muted": True, "zoom": 2.0}}})
            self.assert_equal(events.wait_for("state"), {"data": {"muted": True, "zoom": 2.0}})
            device.event({"state": {"data": {"zoom": 3.0}}})
            self.assert_equal(events.wait_for("state"), {"data": {"zoom": 3.0}})
            with EventStream(self.dashboard, path) as second:
                name, snapshot = second.next()
                assert isinstance(snapshot, dict)
                self.assert_equal([name, snapshot["connected"]], ["snapshot", True])
                self.assert_equal(
                    [snapshot["state"].get("muted"), snapshot["state"].get("zoom")],
                    [True, 3.0],
                    "the second watcher gets the merged state",
                )
            device.event(
                {"status": {"topRight": {"bitrateInfo": {"bitsPerSecond": 8000, "totalBytes": 100}}}}
            )
            status = events.wait_for("status")
            assert isinstance(status, dict)
            self.assert_equal([status["sample"]["bitrate"], status["sample"]["totalBytes"]], [8000, 100])
            device.close(1000)
            self.assert_equal(events.wait_for("connection"), {"connected": False})
        time.sleep(1)
        device = self.connect("E1")
        time.sleep(2)
        self.assert_false(
            any("startStatus" in request for request in device.requests),
            "status is only asked for while someone watches",
        )


def tests(context: Context):
    return [
        DevicesNeedAnId(context),
        DevicesNamed(context),
        DevicesAtMostFive(context),
        DevicesReconnecting(context),
        DevicesReconnectingCount(context),
        DevicesCloseHandshake(context),
        DevicesAtMostTwenty(context),
        DevicesProtocol(context),
        DevicesWheelDevice(context),
        DevicesControls(context),
        DevicesEvents(context),
    ]
