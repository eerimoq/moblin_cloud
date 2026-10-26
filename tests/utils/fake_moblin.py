"""Pretends to be Moblin's remote control streamer and Moblin Cloud, for running the system tests without a device."""

import argparse
import base64
import io
import json
import logging
import random
import threading
import uuid
import zipfile

from websockets.sync.client import ClientConnection
from websockets.sync.client import connect

LOGGER = logging.getLogger(__name__)


def wheel_of(options: list[dict] | None) -> dict | None:
    if options is None:
        return None
    return {"options": [{"text": option["text"], "weight": option["weight"]} for option in options]}


class FakeMoblin:
    def __init__(
        self,
        url: str | None,
        api_url: str,
        key: str | None = None,
        device_id: str | None = None,
    ):
        self.url = url
        self.api_url = api_url
        self.key = key
        self.device_id = device_id or str(uuid.uuid4()).upper()
        self.wheel: dict | None = None
        self.icon = "AppIcon"
        self.ids = {
            name: str(uuid.uuid4()).upper()
            for name in ["C1", "C2", "B1", "B2", "Z1", "Z2", "P1", "P2", "S1", "S2", "A1"]
        }
        self.state = {
            "scene": self.ids["C1"],
            "autoSceneSwitcher": {},
            "mic": "built-in",
            "bitrate": self.ids["B1"],
            "zoom": 1.0,
            "zoomPresets": [{"id": self.ids["Z1"], "name": "1x"}, {"id": self.ids["Z2"], "name": "2x"}],
            "zoomPreset": self.ids["Z1"],
            "debugLogging": False,
            "streaming": True,
            "recording": False,
            "muted": False,
            "stealthMode": False,
            "torchOn": False,
        }
        self.priorities_enabled = False
        self.priorities = [
            {"id": self.ids["P1"], "name": "Cellular", "priority": 1, "enabled": True},
            {"id": self.ids["P2"], "name": "WiFi", "priority": 1, "enabled": True},
        ]
        self.controls: list[dict] = []
        self.refuse_tips = False
        self._status_requested = threading.Event()
        self._status_thread: threading.Thread | None = None
        self.handled_request_ids: set[str] = set()
        self._stopped = threading.Event()
        self._lock = threading.Lock()
        self._websockets: dict[str, ClientConnection] = {}
        self._threads = [
            threading.Thread(target=self.run, args=("control", self.control_url), daemon=True),
            threading.Thread(target=self.run, args=("cloud", self.cloud_url), daemon=True),
        ]

    def start(self):
        for thread in self._threads:
            thread.start()

    def stop(self):
        self._stopped.set()
        self._status_requested.clear()
        self.close("control")
        self.close("cloud")
        for thread in self._threads:
            thread.join(timeout=5)

    def close(self, name: str):
        with self._lock:
            websocket = self._websockets.get(name)
        if websocket is not None:
            websocket.close()

    def control_url(self) -> str | None:
        return self.url

    def cloud_url(self) -> str | None:
        if self.key is None:
            return None
        return f"{self.api_url.replace('http', 'ws', 1)}/remote-control/streamer/{self.key}"

    def run(self, name: str, url_of):
        while not self._stopped.is_set():
            url = url_of()
            if url is not None:
                try:
                    with connect(url, max_size=None, open_timeout=10) as websocket:
                        with self._lock:
                            self._websockets[name] = websocket
                        LOGGER.debug("Connected to %s", url)
                        self.serve(name, websocket)
                except Exception as error:
                    LOGGER.debug("Disconnected from %s: %s", url, error)
                finally:
                    with self._lock:
                        self._websockets.pop(name, None)
                    if name == "cloud":
                        self._status_requested.clear()
            self._stopped.wait(1)

    def serve(self, name: str, websocket: ClientConnection):
        for text in websocket:
            message = json.loads(text)
            if "hello" in message:
                self.send(websocket, {"identify": {"streamerId": self.device_id, "authentication": "fake"}})
            elif "identified" in message:
                if name == "cloud":
                    self.send(websocket, {"event": {"data": {"wheelOfLuck": {"wheel": self.wheel}}}})
                    self.send(websocket, {"event": {"data": {"icon": {"id": self.icon}}}})
            elif "request" in message:
                request = message["request"]
                result, data = self.handle(request["data"])
                response = {"id": request["id"], "result": result}
                if data is not None:
                    response["data"] = data
                self.send(websocket, {"response": response})

    def change_wheel(self, options: list[dict] | None):
        self.wheel = wheel_of(options)
        with self._lock:
            websocket = self._websockets.get("cloud")
        if websocket is not None:
            self.send(websocket, {"event": {"data": {"wheelOfLuck": {"wheel": self.wheel}}}})

    def change_icon(self, icon: str):
        self.icon = icon
        with self._lock:
            websocket = self._websockets.get("cloud")
        if websocket is not None:
            self.send(websocket, {"event": {"data": {"icon": {"id": self.icon}}}})

    def send(self, websocket: ClientConnection, message: dict):
        with self._lock:
            websocket.send(json.dumps(message))

    def send_event(self, data: dict):
        with self._lock:
            websocket = self._websockets.get("cloud")
        if websocket is not None:
            self.send(websocket, {"event": {"data": data}})

    def change_state(self, **changes):
        self.state.update(changes)
        if self._status_requested.is_set():
            self.send_event({"state": {"data": changes}})

    def settings(self) -> dict:
        return {
            "streams": [{"id": self.ids["S1"], "name": "Twitch"}, {"id": self.ids["S2"], "name": "Kick"}],
            "scenes": [{"id": self.ids["C1"], "name": "Main"}, {"id": self.ids["C2"], "name": "BRB"}],
            "autoSceneSwitchers": [{"id": self.ids["A1"], "name": "Evening"}],
            "bitratePresets": [
                {"id": self.ids["B1"], "bitrate": 5000000},
                {"id": self.ids["B2"], "bitrate": 2500000},
            ],
            "mics": [{"id": "built-in", "name": "Built-in"}, {"id": "bt", "name": "Headset"}],
            "srt": {
                "connectionPrioritiesEnabled": self.priorities_enabled,
                "connectionPriorities": self.priorities,
            },
        }

    def run_status(self):
        while self._status_requested.is_set() and not self._stopped.is_set():
            self.send_event({"status": {"topRight": self.status()["topRight"]}})
            self._stopped.wait(1)

    def handle(self, data: dict) -> tuple[dict, dict | None]:
        kind, body = next(iter(data.items()))
        if kind == "importSettings":
            self.import_settings(body["data"])
        elif kind == "getStatus":
            return {"ok": {}}, {"getStatus": self.status()}
        elif kind == "getSettings":
            return {"ok": {}}, {"getSettings": {"data": self.settings()}}
        elif kind == "startStatus":
            self.send_event({"state": {"data": self.state}})
            self._status_requested.set()
            if self._status_thread is None or not self._status_thread.is_alive():
                self._status_thread = threading.Thread(target=self.run_status, daemon=True)
                self._status_thread.start()
        elif kind == "stopStatus":
            self._status_requested.clear()
        elif kind == "tip" and "message" in body["kind"] and self.refuse_tips:
            LOGGER.debug("Refusing tip %s", body)
            return {"error": {}}, None
        elif kind == "tip":
            request_id = body.get("requestId")
            if request_id in self.handled_request_ids:
                LOGGER.debug("Request %s already handled", request_id)
            else:
                if request_id is not None:
                    self.handled_request_ids.add(request_id)
                LOGGER.debug("Got %s %s", kind, body)
        else:
            self.control(kind, body)
        return {"ok": {}}, None

    def control(self, kind: str, body: dict):
        LOGGER.debug("Control %s %s", kind, body)
        self.controls.append({kind: body})
        changes = {
            "setMute": lambda: {"muted": body["on"]},
            "setLive": lambda: {"streaming": body["on"]},
            "setRecord": lambda: {"recording": body["on"]},
            "setStealthMode": lambda: {"stealthMode": body["on"]},
            "setTorch": lambda: {"torchOn": body["on"]},
            "setZoom": lambda: {"zoom": body["x"]},
            "setZoomPreset": lambda: {"zoomPreset": body["id"].upper()},
            "setScene": lambda: {"scene": body["id"].upper()},
            "setMic": lambda: {"mic": body["id"]},
            "setBitratePreset": lambda: {"bitrate": body["id"].upper()},
        }
        if kind in changes:
            self.change_state(**changes[kind]())
        elif kind == "setSrtConnectionPrioritiesEnabled":
            self.priorities_enabled = body["enabled"]
        elif kind == "setSrtConnectionPriority":
            for priority in self.priorities:
                if priority["id"] == body["id"].upper():
                    priority["priority"] = max(1, min(10, body["priority"]))
                    priority["enabled"] = body["enabled"]

    def status(self) -> dict:
        live = self.state["streaming"]
        top_right: dict = {
            "audioInfo": {
                "audioLevel": {"muted": {}} if self.state["muted"] else {"value": {"_0": -23.5}},
                "numberOfAudioChannels": 1,
            },
        }
        if live:
            cellular = 400000 + random.randint(-50000, 50000)
            wifi = 250000 + random.randint(-50000, 50000)
            bits = 8 * (cellular + wifi)
            top_right.update(
                {
                    "bitrate": {"message": f"{bits / 1e6:.1f} Mbps (1.2 GB)", "ok": True},
                    "bitrateInfo": {"bitsPerSecond": bits, "totalBytes": 1200000000},
                    "uptime": {"message": "1:02:03", "ok": True},
                    "srtla": {"message": "62% Cellular, 38% WiFi", "ok": True},
                    "srtlaRtts": {"message": "45 ms Cellular, 30 ms WiFi", "ok": True},
                    "bondingInfo": [
                        {"name": "Cellular", "bytesPerSecond": cellular, "rtt": 40 + random.randint(0, 10)},
                        {"name": "WiFi", "bytesPerSecond": wifi, "rtt": 25 + random.randint(0, 10)},
                    ],
                }
            )
        if self.state["recording"]:
            top_right["recording"] = {"message": "0:10:00", "ok": True}
        return {
            "general": {
                "batteryLevel": 87,
                "batteryCharging": False,
                "flame": "White",
                "wiFiSsid": "Fake",
                "isLive": live,
                "isRecording": self.state["recording"],
                "isMuted": self.state["muted"],
            },
            "topLeft": {
                "stream": {"message": "Twitch", "ok": True},
                "camera": {"message": "Back", "ok": True},
                "mic": {"message": "Built-in", "ok": True},
                "zoom": {"message": f"{self.state['zoom']:.1f}x", "ok": True},
                "viewers": {"message": "12", "ok": True},
            },
            "topRight": top_right,
        }

    def import_settings(self, data: str):
        archive = zipfile.ZipFile(io.BytesIO(base64.b64decode(data)))
        settings = json.loads(archive.read("settings.json"))
        self.url = settings["remoteControl"]["server"]["url"]
        moblin_cloud = settings["moblinCloud"]
        self.key = moblin_cloud["key"] if moblin_cloud["enabled"] else None
        widgets = settings.get("widgets") or []
        self.wheel = wheel_of(widgets[0]["wheelOfLuck"]["options"] if widgets else None)
        self.close("cloud")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("api_url", help="Where Moblin Cloud connects, like Moblin's MOBLIN_WEBSITE_API_URL.")
    parser.add_argument("--control", help="Where the remote control streamer connects at first.")
    parser.add_argument("--key", help="The Moblin Cloud key, instead of waiting for imported settings.")
    parser.add_argument("--device-id", help="The id Moblin identifies itself with (random by default).")
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    LOGGER.setLevel(logging.DEBUG)
    fake = FakeMoblin(args.control, args.api_url, args.key, args.device_id)
    fake.start()
    threading.Event().wait()


if __name__ == "__main__":
    main()
