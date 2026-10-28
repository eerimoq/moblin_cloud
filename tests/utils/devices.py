import json
import queue
import threading
from collections.abc import Callable

from websockets.exceptions import ConnectionClosed
from websockets.exceptions import InvalidStatus
from websockets.sync.client import connect

from .fake_moblin import FakeMoblin

OK: dict = {"ok": {}}


class RawDevice:
    """Moblin Cloud's WebSocket spoken by hand, to send the backend exactly what a test wants.

    Requests from the backend are answered with answer(data) in the background, by default ok, and
    everything the backend sends is kept in received."""

    def __init__(self, port: int, key: str, device_id: str | None, identify: dict | None = None):
        self.websocket = connect(f"ws://localhost:{port}/remote-control/streamer/{key}", open_timeout=10)
        self.hello = json.loads(self.websocket.recv(timeout=10))
        if identify is None:
            identify = {"authentication": "x"}
            if device_id is not None:
                identify["streamerId"] = device_id
        self.websocket.send(json.dumps({"identify": identify}))
        try:
            self.identified = json.loads(self.websocket.recv(timeout=10))
        except ConnectionClosed:
            self.identified = None
        self.answer: Callable[[dict], tuple[dict, dict | None] | None] = lambda data: (OK, None)
        self.requests: list[dict] = []
        self.received: queue.Queue[dict] = queue.Queue()
        self.closed = threading.Event()
        self._thread = threading.Thread(target=self._read, daemon=True)
        self._thread.start()

    def ok(self) -> bool:
        return self.identified == {"identified": {"result": {"ok": {}}}}

    def _read(self):
        try:
            for text in self.websocket:
                message = json.loads(text)
                if "request" in message:
                    request = message["request"]
                    self.requests.append(request["data"])
                    answer = self.answer(request["data"])
                    if answer is not None:
                        result, data = answer
                        response = {"id": request["id"], "result": result}
                        if data is not None:
                            response["data"] = data
                        self.send({"response": response})
                self.received.put(message)
        except ConnectionClosed:
            pass
        finally:
            self.closed.set()

    def send(self, message: dict | str):
        self.websocket.send(message if isinstance(message, str) else json.dumps(message))

    def event(self, data: dict):
        self.send({"event": {"data": data}})

    def push_wheel(self, texts: list[str] | None):
        wheel = None if texts is None else {"options": [{"text": text, "weight": 1} for text in texts]}
        self.event({"wheelOfLuck": {"wheel": wheel}})

    def wait_for(self, check, timeout: float = 10) -> dict:
        while True:
            message = self.received.get(timeout=timeout)
            if check(message):
                return message

    def tips(self) -> list[dict]:
        return [request["tip"] for request in list(self.requests) if "tip" in request]

    def close(self, code: int = 1000):
        self.websocket.close(code)
        self._thread.join(timeout=5)


def refused(port: int, key: str) -> bool:
    try:
        with connect(f"ws://localhost:{port}/remote-control/streamer/{key}", open_timeout=10):
            return False
    except InvalidStatus as error:
        return error.response.status_code == 404


class RecordingMoblin(FakeMoblin):
    """The fake Moblin's Moblin Cloud alone, connected straight to the backend as one device, keeping
    every tip request it accepted."""

    def __init__(self, backend_url: str, key: str, device_id: str, wheel: list[str] | None = None):
        super().__init__(None, backend_url, key=key, device_id=device_id)
        self.tips: list[dict] = []
        if wheel is not None:
            self.wheel = {"options": [{"text": text, "weight": 1} for text in wheel]}

    def handle(self, data: dict) -> tuple[dict, dict | None]:
        result, response = super().handle(data)
        if "tip" in data and "ok" in result:
            self.tips.append(data["tip"])
        return result, response

    def kinds(self, kind: str) -> list[dict]:
        return [tip for tip in list(self.tips) if kind in tip["kind"]]

    def tip_with_id(self, request_id: str) -> dict | None:
        return next((tip for tip in list(self.tips) if tip.get("requestId") == request_id), None)

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()
