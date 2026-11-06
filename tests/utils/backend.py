import logging
import os
import re
import shutil
import signal
import socket
import subprocess
import threading
from collections.abc import Iterable

import requests
from systest import ManagedProcess

from .config import FRONTEND_URL
from .config import Config
from .fake_kick import FakeKick
from .fake_paypal import FakePayPal
from .fake_stripe import FakeStripe
from .fake_twitch import FakeTwitch
from .fake_twitch import user_id_of as twitch_user_id_of
from .utils import FILES_DIR
from .utils import REPO_DIR
from .utils import wait_until

LOGGER = logging.getLogger(__name__)
LOGGER_PROCESS = logging.getLogger(__name__ + ".process")
BACKEND_DIR = REPO_DIR / "backend"
BINARY = BACKEND_DIR / "target" / "release" / "moblin-cloud-backend"
COVERAGE_TARGET_DIR = BACKEND_DIR / "target" / "coverage"
COVERAGE_BINARY = COVERAGE_TARGET_DIR / "release" / "moblin-cloud-backend"
COVERAGE_DIR = FILES_DIR / "coverage"
SESSION_SECRET = "moblin-cloud-system-tests"


def build_backend(coverage: bool):
    LOGGER.debug("Building the backend...")
    command = ["cargo", "build", "--release", "--quiet"]
    if coverage:
        command = [
            "cargo",
            "rustc",
            "--release",
            "--quiet",
            "--target-dir",
            str(COVERAGE_TARGET_DIR),
            "--",
            "-C",
            "instrument-coverage",
        ]
    subprocess.run(command, cwd=BACKEND_DIR, check=True)


class Backend:
    def __init__(self, config: Config, coverage: bool):
        self.config = config
        self.coverage = coverage
        self.binary = COVERAGE_BINARY if coverage else BINARY
        self.environment = {**os.environ, "RUST_LOG": "info"}
        if coverage:
            self.environment["LLVM_PROFILE_FILE"] = str(COVERAGE_DIR / "%p.profraw")
        self.port = config.backend_port()
        self.url = f"http://localhost:{self.port}"
        self.files_dir = FILES_DIR / "backend"
        self._lines_lock = threading.Lock()
        self._lines: list[str] = []
        self._process: ManagedProcess | None = None
        self.fake_twitch = FakeTwitch()
        self.fake_kick = FakeKick()
        self.fake_paypal = FakePayPal()
        self.fake_stripe = FakeStripe()

    def __enter__(self):
        self.fake_paypal.start()
        self.fake_stripe.start()
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()
        self.fake_paypal.stop()
        self.fake_stripe.stop()

    def start(self, without: Iterable[str] = (), extra: Iterable[str] = ()):
        """Starts the backend, without the given options, like "--session-secret", and with the extra
        arguments, for the tests of how it is set up."""
        with socket.socket() as sock:
            if sock.connect_ex(("127.0.0.1", self.port)) == 0:
                raise Exception(f"Port {self.port} is in use, maybe by a backend left from an earlier run.")
        self.files_dir.mkdir(parents=True, exist_ok=True)
        self.fake_twitch = FakeTwitch()
        self.fake_twitch.start()
        self.fake_kick = FakeKick()
        self.fake_kick.start()
        self._process = ManagedProcess(
            [str(self.binary), *self._arguments(without), *extra],
            LOGGER_PROCESS,
            env=self.environment,
            observer=self._handle_line,
            ready=self._wait_until_ready,
        )
        self._process.start()

    def stop(self, graceful: bool = False):
        """Kills the backend, or asks it to stop like a deploy does when graceful or measuring coverage."""
        if self._process is not None:
            process = self._process
            pid = process.pid()
            if (graceful or self.coverage) and pid is not None:
                os.kill(pid, signal.SIGTERM)
                wait_until(lambda: not process.is_running(), "the backend to stop", timeout=30)
            process.stop()
            self._process = None
        self.fake_twitch.stop()
        self.fake_kick.stop()

    def restart(self, without: Iterable[str] = (), extra: Iterable[str] = (), graceful: bool = False):
        self.stop(graceful)
        self.start(without, extra)

    def run(self, without: Iterable[str] = (), extra: Iterable[str] = ()) -> subprocess.CompletedProcess:
        """Runs another backend until it exits, for options it must refuse to start with."""
        return subprocess.run(
            [str(self.binary), *self._arguments(without), *extra],
            env=self.environment,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )

    def wipe(self):
        self.stop()
        shutil.rmtree(self.files_dir, ignore_errors=True)
        self.start()

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

    def _arguments(self, without: Iterable[str]) -> list[str]:
        paypal = self.fake_paypal.config
        stripe = self.fake_stripe.config
        options = [
            ["--listen", f"0.0.0.0:{self.port}"],
            ["--backend-url", self.url],
            ["--frontend-url", FRONTEND_URL],
            ["--session-secret", SESSION_SECRET],
            ["--database", str(self.files_dir / "database.sqlite")],
            ["--admins", twitch_user_id_of("admin")],
            ["--premium-accounts", twitch_user_id_of("pagelook")],
            ["--allow-unattested"],
            ["--delivery-retry-interval", "1"],
            ["--access-recheck-interval", "1"],
            ["--moblin-answer-timeout", "2"],
            ["--twitch-sign-in-url", self.fake_twitch.url],
            ["--kick-sign-in-url", self.fake_kick.url],
            ["--paypal-url", self.fake_paypal.url],
            ["--paypal-client-id", paypal["client-id"]],
            ["--paypal-client-secret", paypal["client-secret"]],
            ["--paypal-partner-id", paypal["partner-id"]],
            ["--paypal-webhook-id", paypal["webhook-id"]],
            ["--stripe-url", self.fake_stripe.url],
            ["--stripe-secret-key", stripe["secret-key"]],
            ["--stripe-publishable-key", stripe["publishable-key"]],
        ]
        without = set(without)
        return [argument for option in options if option[0] not in without for argument in option]

    def _wait_until_ready(self):
        def check() -> bool:
            if self._process is not None and not self._process.is_running():
                raise Exception("The backend exited. See the log for why.")
            try:
                return requests.get(f"{self.url}/icons/plain", timeout=2).ok
            except requests.ConnectionError:
                return False

        wait_until(check, "the backend to answer", timeout=30)
