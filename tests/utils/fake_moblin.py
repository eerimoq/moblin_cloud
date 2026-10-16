"""Pretends to be Moblin's remote control streamer, for running the system tests without a device."""

import argparse
import base64
import io
import json
import logging
import threading
import zipfile
from urllib.parse import parse_qs
from urllib.parse import urlsplit

from websockets.sync.client import ClientConnection
from websockets.sync.client import connect

LOGGER = logging.getLogger(__name__)


def song_id_of(link: str) -> str:
    parts = urlsplit(link)
    return parse_qs(parts.query).get("i", [parts.path.rstrip("/").split("/")[-1]])[0]


def found(link: str) -> bool:
    return not song_id_of(link).startswith("9999")


def wheel_of(options: list[dict] | None) -> dict | None:
    if options is None:
        return None
    return {"options": [{"text": option["text"], "weight": option["weight"]} for option in options]}


class FakeMoblin:
    def __init__(self, url: str):
        self.url = url
        self.wheel: dict | None = None
        self.songs: list[dict] = []
        self.next_song_id = 0
        self.handled_request_ids: set[str] = set()
        self._stopped = threading.Event()
        self._websocket: ClientConnection | None = None
        self._thread = threading.Thread(target=self.run, daemon=True)

    def start(self):
        self._thread.start()

    def stop(self):
        self._stopped.set()
        websocket = self._websocket
        if websocket is not None:
            websocket.close()
        self._thread.join(timeout=5)

    def run(self):
        while not self._stopped.is_set():
            try:
                with connect(self.url, max_size=None, open_timeout=10) as websocket:
                    self._websocket = websocket
                    LOGGER.info("Connected to %s", self.url)
                    self.serve(websocket)
            except Exception as error:
                LOGGER.info("Disconnected from %s: %s", self.url, error)
            finally:
                self._websocket = None
            self._stopped.wait(1)

    def serve(self, websocket: ClientConnection):
        for text in websocket:
            message = json.loads(text)
            if "hello" in message:
                self.send(websocket, {"identify": {"streamerId": "FAKE", "authentication": "fake"}})
            elif "identified" in message:
                self.send(websocket, {"event": {"data": {"wheelOfLuck": {"wheel": self.wheel}}}})
            elif "request" in message:
                request = message["request"]
                result, data = self.handle(request["data"])
                response = {"id": request["id"], "result": result}
                if data is not None:
                    response["data"] = data
                self.send(websocket, {"response": response})

    def change_wheel(self, options: list[dict] | None):
        self.wheel = wheel_of(options)
        websocket = self._websocket
        if websocket is not None:
            self.send(websocket, {"event": {"data": {"wheelOfLuck": {"wheel": self.wheel}}}})

    def send(self, websocket: ClientConnection, message: dict):
        websocket.send(json.dumps(message))

    def handle(self, data: dict) -> tuple[dict, dict | None]:
        kind, body = next(iter(data.items()))
        if kind == "importSettings":
            self.import_settings(body["data"])
        elif kind == "music":
            return self.music(body["command"])
        elif kind in ["tip", "spinWheelOfLuck"]:
            request_id = body.get("requestId")
            if request_id in self.handled_request_ids:
                LOGGER.info("Request %s already handled", request_id)
            else:
                if request_id is not None:
                    self.handled_request_ids.add(request_id)
                LOGGER.info("Got %s %s", kind, body)
        return {"ok": {}}, None

    def import_settings(self, data: str):
        archive = zipfile.ZipFile(io.BytesIO(base64.b64decode(data)))
        settings = json.loads(archive.read("settings.json"))
        self.url = settings["remoteControl"]["server"]["url"]
        widgets = settings.get("widgets") or []
        self.wheel = wheel_of(widgets[0]["wheelOfLuck"]["options"] if widgets else None)

    def music(self, command: dict) -> tuple[dict, dict | None]:
        kind, body = next(iter(command.items()))
        if kind == "find":
            if not found(body["title"]):
                return {"ok": {}}, {"musicFind": {}}
            song_id = song_id_of(body["title"])
            song = {"id": song_id, "title": f"Song {song_id}", "artist": "Fake Artist", "duration": 200}
            return {"ok": {}}, {"musicFind": {"song": song}}
        song_not_found = None
        if kind == "add":
            request_id = body.get("requestId")
            if request_id is not None and any(song.get("requestId") == request_id for song in self.songs):
                song_not_found = False
            elif not found(body["title"]):
                song_not_found = True
            else:
                self.next_song_id += 1
                self.songs.append(
                    {
                        "id": str(self.next_song_id),
                        "title": f"Song {song_id_of(body['title'])}",
                        "artist": "Fake Artist",
                        "duration": 200,
                        "user": body.get("user", "Remote control assistant"),
                        "requestId": request_id,
                    }
                )
                song_not_found = False
        elif kind == "clear":
            self.songs.clear()
        return {"ok": {}}, {"music": {"status": self.status(song_not_found)}}

    def status(self, song_not_found: bool | None) -> dict:
        songs = [{key: value for key, value in song.items() if key != "requestId"} for song in self.songs]
        status: dict = {"playing": bool(songs), "songs": songs}
        if songs:
            status["currentSongIndex"] = 0
        if song_not_found is not None:
            status["songNotFound"] = song_not_found
        return status


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("url", help="Where the remote control streamer connects at first.")
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    FakeMoblin(args.url).run()


if __name__ == "__main__":
    main()
