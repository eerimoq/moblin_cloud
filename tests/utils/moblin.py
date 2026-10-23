import json
import logging
import re
import secrets
import tempfile
import threading
import uuid
import zipfile
from base64 import b64encode
from dataclasses import dataclass
from dataclasses import field
from pathlib import Path

from systest import wait_until
from websockets.exceptions import ConnectionClosed
from websockets.exceptions import InvalidStatus
from websockets.sync.client import ClientConnection
from websockets.sync.client import connect
from websockets.sync.server import Server
from websockets.sync.server import ServerConnection
from websockets.sync.server import serve

from .backend import Backend
from .config import REMOTE_CONTROL_PASSWORD
from .config import WEB_REMOTE_CONTROL_PORT
from .config import Config
from .fake_moblin import FakeMoblin
from .utils import manual_confirmation

LOGGER = logging.getLogger(__name__)
LOGGER_TRAFFIC = logging.getLogger(__name__ + ".traffic")
RE_STREAMER_PATH = re.compile(r"^/remote-control/streamer/([^/?]+)$")
FIRST_OWN_REQUEST_ID = 1_000_000_000


class MoblinError(Exception):
    pass


@dataclass
class BackendRequest:
    """A request the backend sent to Moblin, with Moblin's response once it came."""

    id: int
    data: dict
    response: dict | None = None
    answered: threading.Event = field(default_factory=threading.Event)

    @property
    def kind(self) -> str:
        return next(iter(self.data))

    @property
    def body(self) -> dict:
        return self.data[self.kind]

    def ok(self) -> bool:
        return self.response is not None and "ok" in self.response["result"]


class StreamerConnection:
    """Moblin's remote control streamer connection, either relayed to the backend or ended here."""

    def __init__(self, moblin: "Moblin", connection: ServerConnection, key: str | None):
        self.moblin = moblin
        self.connection = connection
        self.key = key
        self.upstream: ClientConnection | None = None
        self.identified = threading.Event()
        self._own_lock = threading.Lock()
        self._own: dict[int, tuple[threading.Event, list]] = {}

    @property
    def relayed(self) -> bool:
        return self.upstream is not None

    def serve(self):
        if self.key is None:
            self._serve_locally()
            return
        try:
            self.upstream = connect(
                f"ws://localhost:{self.moblin.backend.port}/remote-control/streamer/{self.key}",
                max_size=None,
                open_timeout=10,
            )
        except InvalidStatus as error:
            if error.response.status_code != 404:
                raise
            LOGGER.debug(
                "The backend does not know Moblin Cloud key %s, closing Moblin's connection", self.key
            )
            self.moblin.add_rejected_key(self.key)
            return
        except OSError:
            LOGGER.debug("The backend is down, closing Moblin's connection so it retries")
            return
        self._serve_relayed()

    def _serve_locally(self):
        self._send_to_moblin(
            {
                "hello": {
                    "apiVersion": "0.1",
                    "authentication": {
                        "challenge": secrets.token_hex(64),
                        "salt": secrets.token_hex(64),
                    },
                }
            }
        )
        for text in self.connection:
            message = json.loads(text)
            LOGGER_TRAFFIC.debug("moblin -> tester: %s", text)
            if "identify" in message:
                self._send_to_moblin({"identified": {"result": {"ok": {}}}})
                self.identified.set()
            elif "ping" in message:
                self._send_to_moblin({"pong": {}})
            else:
                self._handle_from_moblin(message)

    def _serve_relayed(self):
        assert self.upstream is not None
        thread = threading.Thread(target=self._relay_from_backend, daemon=True)
        thread.start()
        try:
            for text in self.connection:
                message = json.loads(text)
                if self._handle_from_moblin(message):
                    continue
                LOGGER_TRAFFIC.debug("moblin -> backend: %s", text)
                self.upstream.send(text)
        except ConnectionClosed:
            pass
        finally:
            self.upstream.close()
            thread.join(timeout=5)

    def _relay_from_backend(self):
        assert self.upstream is not None
        try:
            for text in self.upstream:
                LOGGER_TRAFFIC.debug("backend -> moblin: %s", text)
                message = json.loads(text)
                if "identified" in message:
                    self.identified.set()
                elif "request" in message:
                    self.moblin.add_backend_request(
                        BackendRequest(message["request"]["id"], message["request"]["data"])
                    )
                self.connection.send(text)
        except ConnectionClosed:
            pass
        finally:
            self.connection.close()

    def _handle_from_moblin(self, message: dict) -> bool:
        """Handles what Moblin sends, returning True for answers to the tester's own requests."""
        if "response" in message:
            response = message["response"]
            with self._own_lock:
                own = self._own.pop(response["id"], None)
            if own is not None:
                own[1].append(response)
                own[0].set()
                return True
            if self.relayed:
                self.moblin.answer_backend_request(response)
        elif "event" in message:
            self.moblin.handle_event(message["event"]["data"])
        return False

    def fail_own_requests(self):
        with self._own_lock:
            own = list(self._own.values())
            self._own.clear()
        for answered, _ in own:
            answered.set()

    def request(self, data: dict, timeout: float) -> dict:
        request_id = self.moblin.next_request_id()
        answered = threading.Event()
        responses: list[dict] = []
        with self._own_lock:
            self._own[request_id] = (answered, responses)
        self._send_to_moblin({"request": {"id": request_id, "data": data}})
        if not answered.wait(timeout):
            with self._own_lock:
                self._own.pop(request_id, None)
            raise MoblinError(f"Moblin did not answer {next(iter(data))} in {timeout} seconds.")
        if not responses:
            raise MoblinError(f"Moblin disconnected before answering {next(iter(data))}.")
        return responses[0]

    def _send_to_moblin(self, message: dict):
        text = json.dumps(message)
        LOGGER_TRAFFIC.debug("tester -> moblin: %s", text[:1000])
        self.connection.send(text)


class Moblin:
    """A Moblin device, or the fake one in tests/utils/fake_moblin.py, connected to the tester twice.

    Its remote control streamer connects to the tester, which acts as Moblin's assistant and
    sends its own requests to Moblin, like moblin_assistant does, to set it up and look at it.
    Its Moblin Cloud connection, made when it has a Moblin Cloud key from the dashboard, is relayed
    to the backend, so the backend talks to the real Moblin while the tester watches.
    """

    def __init__(self, config: Config, backend: Backend, device_name: str | None):
        self.config = config
        self.backend = backend
        self.device_name = device_name if device_name is not None else "fake Moblin"
        self._fake: FakeMoblin | None = None
        self._port = config.remote_control_port()
        self._tester_ip_address = config.tester_ip_address()
        self._lock = threading.Lock()
        self._connected = threading.Condition(self._lock)
        self._control: StreamerConnection | None = None
        self._cloud: StreamerConnection | None = None
        self._rejected_keys: list[str] = []
        self._next_request_id = FIRST_OWN_REQUEST_ID
        self._backend_requests: list[BackendRequest] = []
        self._log_entries: list[str] = []
        self._wheel: dict | None = None
        self._server: Server | None = None
        self._thread: threading.Thread | None = None

    def __enter__(self):
        self._server = serve(self._handle_connection, "0.0.0.0", self._port, max_size=None)
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)
        self._thread.start()
        if self.is_fake():
            self._fake = FakeMoblin(self.remote_control_url(), self.api_url())
            self._fake.start()
        if not self.is_fake():
            LOGGER.info(
                "Waiting for %s's remote control streamer to connect to %s...",
                self.device_name,
                self.remote_control_url(),
            )
        self.wait_for_control_connection(timeout=300)
        if not self.is_fake():
            LOGGER.info("Remote control streamer connected")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self._fake is not None:
            self._fake.stop()
        if self._server is not None:
            self._server.shutdown()
        if self._thread is not None:
            self._thread.join(timeout=5)

    def _handle_connection(self, connection: ServerConnection):
        assert connection.request is not None
        mo = RE_STREAMER_PATH.match(connection.request.path)
        streamer = StreamerConnection(self, connection, mo.group(1) if mo else None)
        LOGGER.debug("Moblin connected to %s", connection.request.path)
        with self._lock:
            if streamer.key is None:
                self._control = streamer
            else:
                self._cloud = streamer
        identifier = threading.Thread(target=self._notify_when_identified, args=(streamer,), daemon=True)
        identifier.start()
        try:
            streamer.serve()
        except ConnectionClosed:
            pass
        finally:
            streamer.fail_own_requests()
            with self._connected:
                if self._control is streamer:
                    self._control = None
                if self._cloud is streamer:
                    self._cloud = None
                self._connected.notify_all()
            LOGGER.debug("Moblin disconnected from %s", connection.request.path)

    def _notify_when_identified(self, streamer: StreamerConnection):
        if streamer.identified.wait(30):
            with self._connected:
                self._connected.notify_all()

    def next_request_id(self) -> int:
        with self._lock:
            self._next_request_id += 1
            return self._next_request_id

    def add_backend_request(self, request: BackendRequest):
        LOGGER.debug("The backend sent %s %s", request.kind, request.body)
        with self._lock:
            self._backend_requests.append(request)

    def answer_backend_request(self, response: dict):
        with self._lock:
            for request in reversed(self._backend_requests):
                if request.id == response["id"] and request.response is None:
                    request.response = response
                    request.answered.set()
                    return

    def add_rejected_key(self, key: str):
        with self._connected:
            self._rejected_keys.append(key)
            self._connected.notify_all()

    def handle_event(self, data: dict):
        with self._lock:
            if "log" in data:
                self._log_entries.append(data["log"]["entry"])
            elif "wheelOfLuck" in data:
                self._wheel = data["wheelOfLuck"].get("wheel")

    def is_fake(self) -> bool:
        return self.device_name == "fake Moblin"

    def connected_key(self) -> str | None:
        """The key of Moblin's Moblin Cloud connection to the backend, if any."""
        with self._lock:
            connection = self._cloud
        if connection is None or not connection.identified.is_set():
            return None
        return connection.key

    def _wait(self, check, message: str, timeout: float):
        with self._connected:
            if not self._connected.wait_for(check, timeout):
                raise MoblinError(f"Timeout waiting for {message}.")

    def wait_for_control_connection(self, timeout: float = 60, other_than: StreamerConnection | None = None):
        def check() -> bool:
            connection = self._control
            return connection is not None and connection is not other_than and connection.identified.is_set()

        self._wait(check, "Moblin's remote control streamer to connect", timeout)

    def wait_for_backend_connection(
        self, key: str, timeout: float = 60, other_than: StreamerConnection | None = None
    ):
        def check() -> bool:
            connection = self._cloud
            return (
                connection is not None
                and connection is not other_than
                and connection.key == key
                and connection.identified.is_set()
            )

        self._wait(check, "Moblin Cloud to connect to the backend", timeout)

    def wait_for_no_backend_connection(self, timeout: float = 60):
        self._wait(lambda: self._cloud is None, "Moblin Cloud to disconnect from the backend", timeout)

    def wait_for_rejected_key(self, key: str, start: int = 0, timeout: float = 60):
        self._wait(
            lambda: key in self._rejected_keys[start:],
            f"the backend to refuse Moblin Cloud with the key {key}",
            timeout,
        )

    def rejected_keys_position(self) -> int:
        with self._lock:
            return len(self._rejected_keys)

    def request(self, data: dict, timeout: float = 15) -> dict:
        self.wait_for_control_connection()
        with self._lock:
            connection = self._control
        if connection is None:
            raise MoblinError("Moblin is not connected.")
        response = connection.request(data, timeout)
        if "ok" not in response["result"]:
            raise MoblinError(f"{next(iter(data))} failed: {response['result']}")
        return response

    def remote_control_url(self) -> str:
        return f"ws://{self._tester_ip_address}:{self._port}"

    def api_url(self) -> str:
        """Where Moblin Cloud connects, the MOBLIN_WEBSITE_API_URL of a Moblin debug build."""
        return f"http://{self._tester_ip_address}:{self._port}"

    def import_settings(self, key: str | None, wheel: list[dict] | None = None):
        """Imports test settings, which also set where the remote control streamer connects and
        Moblin Cloud's key."""
        settings = settings_for(self.remote_control_url(), key, wheel)
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "settings.zip"
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as zip_file:
                zip_file.writestr("settings.json", json.dumps(settings, indent=4))
            data = b64encode(archive.read_bytes()).decode()
        try:
            self.request({"importSettings": {"data": data}}, timeout=30)
        except (MoblinError, ConnectionClosed) as error:
            LOGGER.debug("No answer to import settings: %s", error)

    def _reconnect(self) -> tuple[StreamerConnection | None, StreamerConnection | None]:
        with self._lock:
            control = self._control
            cloud = self._cloud
        for connection in [control, cloud]:
            if connection is not None:
                connection.connection.close()
        return control, cloud

    def connect_to_backend(self, key: str, wheel: list[dict] | None = None):
        """Gives Moblin the Moblin Cloud key from the dashboard, like a streamer pasting it into Moblin."""
        with self._lock:
            self._wheel = None
        self.import_settings(key, wheel)
        control, cloud = self._reconnect()
        self.wait_for_control_connection(other_than=control)
        self.wait_for_backend_connection(key, other_than=cloud)

    def change_wheel(self, wheel: list[dict] | None):
        """Changes the wheel without reconnecting, like a streamer in Moblin's settings, as Moblin
        reconnects when settings are imported. A person does it on a real device, where the wheel
        can only be removed or set back to the one of the last imported settings."""
        with self._lock:
            connection = self._cloud
        if self._fake is not None:
            self._fake.change_wheel(wheel)
        elif wheel is None:
            manual_confirmation("In Moblin's Moblin Cloud settings, pick no wheel of luck.")
        else:
            manual_confirmation("In Moblin's Moblin Cloud settings, pick the wheel of luck Wheel.")
        with self._lock:
            if self._cloud is not connection:
                raise MoblinError("Moblin reconnected when its wheel changed.")

    def disconnect_from_backend(self):
        if self.connected_key() is None:
            return
        self.import_settings(None)
        control, _ = self._reconnect()
        self.wait_for_control_connection(other_than=control)
        self.wait_for_no_backend_connection()

    def disconnect(self) -> StreamerConnection | None:
        """Closes Moblin Cloud's connection, which makes Moblin connect again, and returns the
        closed connection."""
        with self._lock:
            connection = self._cloud
        if connection is not None:
            connection.connection.close()
        return connection

    def wheel(self) -> dict | None:
        with self._lock:
            return self._wheel

    def backend_requests(self, kind: str | None = None) -> list[BackendRequest]:
        with self._lock:
            return [request for request in self._backend_requests if kind is None or request.kind == kind]

    def backend_requests_position(self) -> int:
        with self._lock:
            return len(self._backend_requests)

    def wait_for_new_backend_request(self, kind: str, start: int, timeout: float = 60) -> BackendRequest:
        """Waits for Moblin to accept a request of a kind sent after start, whatever its request id."""
        found: list[BackendRequest] = []

        def check() -> bool:
            with self._lock:
                requests = self._backend_requests[start:]
            for request in requests:
                if request.kind == kind and request.ok():
                    found.append(request)
                    return True
            return False

        wait_until(check, f"Moblin to accept a {kind} from the backend", timeout=timeout)
        return found[0]

    def wait_for_backend_request(self, kind: str, request_id: str, timeout: float = 60) -> BackendRequest:
        found: list[BackendRequest] = []

        def check() -> bool:
            for request in self.backend_requests(kind):
                if request.body.get("requestId") == request_id and request.ok():
                    found.append(request)
                    return True
            return False

        wait_until(check, f"Moblin to accept {kind} {request_id} from the backend", timeout=timeout)
        return found[0]

    def log_position(self) -> int:
        with self._lock:
            return len(self._log_entries)

    def wait_for_log(self, pattern: str, start: int = 0, timeout: float = 30):
        regex = re.compile(pattern)

        def check() -> bool:
            with self._lock:
                return any(regex.search(entry) for entry in self._log_entries[start:])

        wait_until(check, f"Moblin log entry matching '{pattern}'", timeout=timeout)

    def set_live(self, on: bool):
        self.request({"setLive": {"on": on}})


def wheel_option(text: str, weight: int = 1) -> dict:
    return {"id": str(uuid.uuid4()).upper(), "text": text, "weight": weight}


def settings_for(remote_control_url: str, key: str | None, wheel: list[dict] | None) -> dict:
    remote_control: dict = {
        "server": {
            "enabled": True,
            "url": remote_control_url,
            "reliableChatAndEvents": True,
        },
        "web": {"enabled": True, "port": WEB_REMOTE_CONTROL_PORT},
        "password": REMOTE_CONTROL_PASSWORD,
        "hasMigratedAssistant": True,
    }
    scene = {"name": "Front", "cameraPosition": "Front", "enabled": True}
    moblin_cloud: dict = {"enabled": key is not None, "key": key or ""}
    settings: dict = {
        "moblinCloud": moblin_cloud,
        "scenes": [scene],
        "remoteControl": remote_control,
        "verboseStatuses": True,
        "showAllSettings": True,
        "debug": {"logLevel": "Debug"},
    }
    if wheel is not None:
        widget_id = str(uuid.uuid4()).upper()
        settings["widgets"] = [
            {
                "id": widget_id,
                "name": "Wheel",
                "type": "Wheel of luck",
                "enabled": True,
                "wheelOfLuck": {"advanced": False, "options": wheel},
            }
        ]
        scene["widgets"] = [
            {
                "widgetId": widget_id,
                "alignment": "TopLeft",
                "x": 25,
                "y": 25,
                "size": 50,
                "migrated": True,
                "migrated2": True,
            }
        ]
        moblin_cloud["wheelOfLuckWidgetId"] = widget_id
    return settings
