"""Pretends to be Moblin's remote control streamer and Moblin Cloud, for running the system tests without a device."""

import argparse
import base64
import io
import json
import logging
import random
import struct
import threading
import time
import uuid
import zipfile
import zlib
from typing import Any

from websockets.sync.client import ClientConnection
from websockets.sync.client import connect

from .fake_moblin_database import database

LOGGER = logging.getLogger(__name__)


def wheel_of(options: list[dict] | None) -> dict | None:
    if options is None:
        return None
    return {"options": [{"text": option["text"], "weight": option["weight"]} for option in options]}


def png(width: int, height: int, rows: bytes) -> bytes:
    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(rows, 6))
        + chunk(b"IEND", b"")
    )


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
            for name in [
                "C1",
                "C2",
                "C3",
                "B1",
                "B2",
                "Z1",
                "Z2",
                "P1",
                "P2",
                "S1",
                "S2",
                "A1",
                "M1",
                "M2",
            ]
        }
        self.database = database(self.ids, self.icon)
        self.filters: dict[str, bool] = {}
        self.state: dict[str, Any] = {
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
            "previewStream": False,
            "muted": False,
            "stealthMode": False,
            "torchOn": False,
            "gimbalTracking": False,
            "filters": [],
            "macros": [
                {"id": macro["id"], "name": macro["name"], "running": False}
                for macro in self.database["macros"]["macros"]
            ],
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
        self._preview_requested = threading.Event()
        self._preview_thread: threading.Thread | None = None
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
        self._preview_requested.clear()
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
                        self._preview_requested.clear()
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
        self.database["iconImage"] = icon
        with self._lock:
            websocket = self._websockets.get("cloud")
        if websocket is not None:
            self.send(websocket, {"event": {"data": {"icon": {"id": self.icon}}}})

    def send(self, websocket: ClientConnection, message: dict):
        with self._lock:
            websocket.send(json.dumps(message))

    def send_cloud(self, message: dict):
        with self._lock:
            websocket = self._websockets.get("cloud")
        if websocket is not None:
            try:
                self.send(websocket, message)
            except Exception as error:
                LOGGER.debug("Could not send: %s", error)

    def send_event(self, data: dict):
        self.send_cloud({"event": {"data": data}})

    def change_state(self, **changes):
        self.state.update(changes)
        if self._status_requested.is_set():
            self.send_event({"state": {"data": changes}})

    def enabled_stream(self) -> dict:
        return next(stream for stream in self.database["streams"] if stream["enabled"])

    def settings(self) -> dict:
        return {
            "streams": [{"id": stream["id"], "name": stream["name"]} for stream in self.database["streams"]],
            "scenes": [
                {"id": scene["id"], "name": scene["name"]}
                for scene in self.database["scenes"]
                if scene["enabled"]
            ],
            "autoSceneSwitchers": [
                {"id": switcher["id"], "name": switcher["name"]}
                for switcher in self.database["autoSceneSwitchers"]["switchers"]
            ],
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

    def frame(self) -> bytes:
        width, height = 256, 144
        scene = self.state["scene"]
        top, bottom = (
            ((40, 30, 70), (120, 40, 160)) if scene == self.ids["C2"] else ((70, 130, 200), (40, 90, 50))
        )
        light = 40 if self.state["torchOn"] else 0
        line = int(time.time() * 40) % width
        bar_width = max(2, int(6 * float(self.state["zoom"])))
        canvas = bytearray()
        for y in range(height):
            mix = y / (height - 1)
            pixel = bytes(min(255, int(top[i] + (bottom[i] - top[i]) * mix) + light) for i in range(3))
            canvas += (
                pixel * line
                + b"\xff\xff\xff" * min(bar_width, width - line)
                + pixel * max(0, width - line - bar_width)
            )
        rows = bytearray()
        for y in range(height):
            row = canvas[3 * width * y : 3 * width * (y + 1)]
            if self.filters.get("grayScale") or self.filters.get("sepia"):
                for x in range(width):
                    gray = sum(row[3 * x : 3 * x + 3]) // 3
                    if self.filters.get("sepia"):
                        row[3 * x : 3 * x + 3] = bytes((min(255, gray + 40), min(255, gray + 20), gray))
                    else:
                        row[3 * x : 3 * x + 3] = bytes((gray, gray, gray))
            if self.filters.get("movie") and (y < 18 or y >= height - 18):
                row = bytearray(3 * width)
            rows += b"\x00" + row
        return png(width, height, bytes(rows))

    def run_preview(self):
        while self._preview_requested.is_set() and not self._stopped.is_set():
            self.send_cloud({"preview": {"preview": base64.b64encode(self.frame()).decode()}})
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
        elif kind == "startPreview":
            self._preview_requested.set()
            if self._preview_thread is None or not self._preview_thread.is_alive():
                self._preview_thread = threading.Thread(target=self.run_preview, daemon=True)
                self._preview_thread.start()
        elif kind == "stopPreview":
            self._preview_requested.clear()
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
            return self.control(kind, body)
        return {"ok": {}}, None

    def control(self, kind: str, body: dict) -> tuple[dict, dict | None]:
        LOGGER.debug("Control %s %s", kind, body)
        self.controls.append({kind: body})
        changes = {
            "setMute": lambda: {"muted": body["on"]},
            "setLive": lambda: {"streaming": body["on"]},
            "setRecord": lambda: {"recording": body["on"]},
            "setStealthMode": lambda: {"stealthMode": body["on"]},
            "setTorch": lambda: {"torchOn": body["on"]},
            "setZoom": lambda: {"zoom": body["x"]},
            "setZoomPreset": lambda: self.zoom_preset(body["id"].upper()),
            "setScene": lambda: {"scene": body["id"].upper()},
            "setMic": lambda: {"mic": body["id"]},
            "setBitratePreset": lambda: {"bitrate": body["id"].upper()},
            "setGimbalTracking": lambda: {"gimbalTracking": body["on"]},
            "setPreviewStream": lambda: {"previewStream": body["on"]},
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
        elif kind == "setAutoSceneSwitcher":
            switcher = body.get("id")
            self.database["autoSceneSwitchers"]["switcherId"] = switcher and switcher.upper()
            self.change_state(autoSceneSwitcher={"id": switcher.upper()} if switcher else {})
        elif kind == "setFilter":
            self.filters[next(iter(body["filter"]))] = body["on"]
            pairs: list[Any] = []
            for name, on in self.filters.items():
                pairs += [{name: {}}, on]
            self.change_state(filters=pairs)
        elif kind in ("startMacro", "stopMacro"):
            macros = [
                {
                    **macro,
                    "running": macro["running"]
                    if macro["id"] != body["id"].upper()
                    else kind == "startMacro",
                }
                for macro in self.state["macros"]
            ]
            self.change_state(macros=macros)
        return {"ok": {}}, None

    def zoom_preset(self, preset: str) -> dict:
        zoom = next((item["x"] for item in self.database["zoom"]["back"] if item["id"] == preset), 1.0)
        return {"zoomPreset": preset, "zoom": float(zoom)}

    def status(self) -> dict:
        live = self.state["streaming"]
        top_right: dict = {
            "audioInfo": {
                "audioLevel": {"muted": {}}
                if self.state["muted"]
                else {"value": {"_0": -23.5 + random.uniform(-6, 6)}},
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
                "stream": {"message": self.enabled_stream()["name"], "ok": True},
                "camera": {"message": "Back", "ok": True},
                "mic": {"message": "Built-in" if self.state["mic"] == "built-in" else "Headset", "ok": True},
                "zoom": {"message": f"{self.state['zoom']:.1f}x", "ok": True},
                "chat": {"message": "Connected", "ok": True},
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
    parser.add_argument("api_url", help="Where Moblin Cloud connects, like Moblin's MOBLIN_CLOUD_API_URL.")
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
