import logging
import os
import re
import shutil
import socket
import subprocess
import threading

import requests
from systest import ManagedProcess
from systest import wait_until

from .config import FRONTEND_URL
from .config import Config
from .utils import FILES_DIR
from .utils import REPO_DIR

LOGGER = logging.getLogger(__name__)
LOGGER_PROCESS = logging.getLogger(__name__ + ".process")
BACKEND_DIR = REPO_DIR / "backend"
BINARY = BACKEND_DIR / "target" / "release" / "moblin-website-backend"
SESSION_SECRET = "moblin-website-system-tests"


def build_backend():
    LOGGER.info("Building the backend...")
    subprocess.run(["cargo", "build", "--release", "--quiet"], cwd=BACKEND_DIR, check=True)


class Backend:
    def __init__(self, config: Config):
        self.config = config
        self.port = config.backend_port()
        self.url = f"http://localhost:{self.port}"
        self.files_dir = FILES_DIR / "backend"
        self._lines_lock = threading.Lock()
        self._lines: list[str] = []
        self._process: ManagedProcess | None = None

    def __enter__(self):
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()

    def start(self):
        with socket.socket() as sock:
            if sock.connect_ex(("127.0.0.1", self.port)) == 0:
                raise Exception(f"Port {self.port} is in use, maybe by a backend left from an earlier run.")
        self.files_dir.mkdir(parents=True, exist_ok=True)
        self._process = ManagedProcess(
            [str(BINARY), *self._arguments()],
            LOGGER_PROCESS,
            env={**os.environ, "RUST_LOG": "info"},
            observer=self._handle_line,
            ready=self._wait_until_ready,
        )
        self._process.start()

    def stop(self):
        if self._process is not None:
            self._process.stop()
            self._process = None

    def restart(self):
        self.stop()
        self.start()

    def wipe(self):
        self.stop()
        shutil.rmtree(self.files_dir, ignore_errors=True)
        self.start()

    def device_streamer_url(self, streamer_url: str) -> str:
        """The streamer URL as the Moblin device reaches this backend."""
        return re.sub(
            r"^ws://localhost:\d+", f"ws://{self.config.tester_ip_address()}:{self.port}", streamer_url
        )

    def log_position(self) -> int:
        with self._lines_lock:
            return len(self._lines)

    def wait_for_log(self, pattern: str, start: int = 0, timeout: float = 60) -> re.Match:
        regex = re.compile(pattern)
        found: list[re.Match] = []

        def check() -> bool:
            with self._lines_lock:
                lines = self._lines[start:]
            for line in lines:
                mo = regex.search(line)
                if mo is not None:
                    found.append(mo)
                    return True
            return False

        wait_until(check, f"backend log line matching '{pattern}'", timeout=timeout)
        return found[0]

    def has_logged(self, pattern: str, start: int = 0) -> bool:
        regex = re.compile(pattern)
        with self._lines_lock:
            return any(regex.search(line) for line in self._lines[start:])

    def _handle_line(self, line: str):
        with self._lines_lock:
            self._lines.append(line)

    def _arguments(self) -> list[str]:
        arguments = [
            "--listen",
            f"0.0.0.0:{self.port}",
            "--public-url",
            self.url,
            "--frontend-url",
            FRONTEND_URL,
            "--session-secret",
            SESSION_SECRET,
            "--accounts-file",
            str(self.files_dir / "accounts.json"),
            "--pages-file",
            str(self.files_dir / "pages.json"),
            "--song-queue-file",
            str(self.files_dir / "song-queue.json"),
            "--lookup-spacing",
            "1",
        ]
        if self.config.allow_unattested():
            arguments.append("--allow-unattested")
        else:
            arguments += ["--app-attest-environment", self.config.app_attest_environment()]
            if self.config.app_id() is not None:
                arguments += ["--app-id", self.config.app_id()]
        if self.config.twitch_sign_in() == "fake":
            arguments.append("--fake-twitch-sign-in")
        twitch = self.config.twitch()
        if twitch is not None:
            arguments += [
                "--twitch-client-id",
                twitch["client-id"],
                "--twitch-client-secret",
                twitch["client-secret"],
            ]
            if "live-channel" in twitch:
                arguments += ["--twitch-live-channel", twitch["live-channel"]]
        paypal = self.config.paypal()
        if paypal is not None:
            arguments += [
                "--paypal-sandbox",
                "--paypal-client-id",
                paypal["client-id"],
                "--paypal-client-secret",
                paypal["client-secret"],
                "--paypal-partner-id",
                paypal["partner-id"],
            ]
            if "attribution-id" in paypal:
                arguments += ["--paypal-attribution-id", paypal["attribution-id"]]
            if "webhook-id" in paypal:
                arguments += ["--paypal-webhook-id", paypal["webhook-id"]]
        return arguments

    def _wait_until_ready(self):
        def check() -> bool:
            if self._process is not None and not self._process.is_running():
                raise Exception("The backend exited. See the log for why.")
            try:
                return requests.get(f"{self.url}/streamers", timeout=2).ok
            except requests.ConnectionError:
                return False

        wait_until(check, "the backend to answer", timeout=30)
