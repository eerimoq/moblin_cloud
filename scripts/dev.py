import argparse
import contextlib
import logging
import os
import signal
import sqlite3
import subprocess
import sys
import threading
import time
from pathlib import Path
from types import FrameType

import requests
from tests.utils import fake_moblin
from tests.utils.fake_kick import FakeKick
from tests.utils.fake_moblin import FakeMoblin
from tests.utils.fake_twitch import FakeTwitch

REPO_DIR = Path(__file__).resolve().parent.parent
BACKEND_DIR = REPO_DIR / "backend"
BACKEND_URL = "http://localhost:8080"
DATABASE = os.environ.get("DATABASE", "./database.sqlite")


def _interrupt(signum: int, _frame: FrameType | None):
    raise KeyboardInterrupt(signal.Signals(signum).name)


class Processes:
    def __init__(self):
        self._processes: list[subprocess.Popen] = []

    def __enter__(self):
        for signum in [signal.SIGTERM, signal.SIGHUP]:
            signal.signal(signum, _interrupt)
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()

    def start(self, command: list[str], cwd: Path, env: dict[str, str] | None = None):
        self._processes.append(
            subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL, start_new_session=True)
        )

    def check_running(self):
        for process in self._processes:
            if process.poll() is not None:
                sys.exit(f"{process.args!r} exited with {process.returncode}.")

    def wait(self):
        pid, status = os.wait()
        for process in self._processes:
            if process.pid == pid:
                process.returncode = os.waitstatus_to_exitcode(status)
        self.check_running()

    def stop(self):
        for signum in [signal.SIGINT, signal.SIGTERM, signal.SIGHUP]:
            signal.signal(signum, signal.SIG_IGN)
        for process in self._processes:
            self._kill(process, signal.SIGTERM)
        deadline = time.monotonic() + 5
        for process in self._processes:
            while self._is_running(process) and time.monotonic() < deadline:
                time.sleep(0.1)
            self._kill(process, signal.SIGKILL)
            process.wait()
        self._processes = []

    @staticmethod
    def _is_running(process: subprocess.Popen) -> bool:
        process.poll()
        try:
            os.killpg(process.pid, 0)
        except (ProcessLookupError, PermissionError):
            return False
        return True

    @staticmethod
    def _kill(process: subprocess.Popen, signum: int):
        try:
            os.killpg(process.pid, signum)
        except (ProcessLookupError, PermissionError):
            pass


def parse_platforms(platforms: str) -> list[str]:
    return [platform for platform in platforms.split(",") if platform]


def start_backend(stack: contextlib.ExitStack, processes: Processes, platforms: list[str], args: list[str]):
    sign_in = []
    if "twitch" in platforms:
        stack.enter_context(FakeTwitch(8081))
        sign_in += ["--twitch-sign-in-url", "http://localhost:8081"]
    if "kick" in platforms:
        stack.enter_context(FakeKick(8082))
        sign_in += ["--kick-sign-in-url", "http://localhost:8082"]
    subprocess.run(["cargo", "build"], cwd=BACKEND_DIR, check=True)
    processes.start(
        [
            "cargo",
            "run",
            "-q",
            "--",
            "--listen",
            "0.0.0.0:8080",
            "--allow-unattested",
            *sign_in,
            "--admins",
            "twitch:847161455,kick:33069881",
            "--backend-url",
            "http://localhost:5173/api",
            "--frontend-url",
            "http://localhost:5173",
            *args,
        ],
        BACKEND_DIR,
        {**os.environ, "DATABASE": DATABASE},
    )


def start_frontend(processes: Processes):
    processes.start(["npm", "run", "dev"], REPO_DIR / "frontend")


def admin_key(platform: str) -> str:
    with sqlite3.connect(BACKEND_DIR / DATABASE) as connection:
        row = connection.execute(
            "select moblin_key from accounts join identities on identities.account = accounts.id "
            "where platform = ? and login = 'admin'",
            (platform,),
        ).fetchone()
    if row is None or not row[0]:
        sys.exit(
            "No Moblin Cloud key for the admin user. Sign in as admin at http://localhost:5173/dashboard/ first."
        )
    return row[0]


def start_fake_moblin(
    platform: str, control: str | None = None, device_id: str = "FAKE-MOBLIN"
) -> FakeMoblin:
    fake_moblin.LOGGER.setLevel(logging.DEBUG)
    fake = FakeMoblin(control, BACKEND_URL, admin_key(platform), device_id)
    fake.start()
    return fake


def wait_for_backend(processes: Processes):
    while True:
        processes.check_running()
        try:
            if requests.get(f"{BACKEND_URL}/icons/plain", timeout=2).ok:
                return
        except requests.ConnectionError:
            pass
        time.sleep(0.2)


def sign_in_as_admin(platform: str):
    session = requests.Session()
    response = session.get(f"{BACKEND_URL}/auth/{platform}/login", allow_redirects=False, timeout=10)
    response.raise_for_status()
    response = requests.get(
        response.headers["Location"], params={"login": "admin"}, allow_redirects=False, timeout=10
    )
    response.raise_for_status()
    callback = response.headers["Location"].replace("localhost:5173/api", "localhost:8080")
    session.get(callback, allow_redirects=False, timeout=10).raise_for_status()


def run_all(args: argparse.Namespace, _rest: list[str]):
    platforms = parse_platforms(args.platforms)
    with Processes() as processes, contextlib.ExitStack() as stack:
        start_backend(stack, processes, platforms, [])
        wait_for_backend(processes)
        start_frontend(processes)
        if platforms:
            sign_in_as_admin(platforms[0])
            stack.callback(start_fake_moblin(platforms[0]).stop)
        processes.wait()


def run_backend(args: argparse.Namespace, rest: list[str]):
    with Processes() as processes, contextlib.ExitStack() as stack:
        start_backend(stack, processes, parse_platforms(args.platforms), rest)
        processes.wait()


def run_frontend(_args: argparse.Namespace, _rest: list[str]):
    with Processes() as processes:
        start_frontend(processes)
        processes.wait()


def run_fake_moblin(args: argparse.Namespace, _rest: list[str]):
    start_fake_moblin(args.platform, args.control, args.device_id)
    threading.Event().wait()


def run_mobtop(_args: argparse.Namespace, rest: list[str]):
    os.chdir(REPO_DIR / "mobtop")
    os.execvp("cargo", ["cargo", "run", "-q", "--", "--backend-url", "http://localhost:5173/api", *rest])


def main():
    parser = argparse.ArgumentParser(description="Runs Moblin Cloud locally against the fakes.")
    subparsers = parser.add_subparsers(required=True)

    subparser = subparsers.add_parser("all", help="The backend, the frontend and the fake Moblin.")
    subparser.add_argument("platforms", nargs="?", default="twitch,kick", help="The fake sign in platforms.")
    subparser.set_defaults(func=run_all)

    subparser = subparsers.add_parser("backend", help="The backend. Unknown arguments go to it.")
    subparser.add_argument("--platforms", default="twitch,kick", help="The fake sign in platforms.")
    subparser.set_defaults(func=run_backend, passes_rest=True)

    subparser = subparsers.add_parser("frontend", help="The frontend's dev server.")
    subparser.set_defaults(func=run_frontend)

    subparser = subparsers.add_parser("fake-moblin", help="The fake Moblin as the admin user's Moblin.")
    subparser.add_argument("--platform", default="twitch", help="The platform the admin user signed in with.")
    subparser.add_argument("--control", help="Where the remote control streamer connects at first.")
    subparser.add_argument(
        "--device-id", default="FAKE-MOBLIN", help="The id the fake Moblin identifies itself with."
    )
    subparser.set_defaults(func=run_fake_moblin)

    subparser = subparsers.add_parser("mobtop", help="mobtop. Unknown arguments go to it.")
    subparser.set_defaults(func=run_mobtop, passes_rest=True)

    args, rest = parser.parse_known_args()
    if rest and not getattr(args, "passes_rest", False):
        parser.error(f"unrecognized arguments: {' '.join(rest)}")
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    try:
        args.func(args, rest)
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
