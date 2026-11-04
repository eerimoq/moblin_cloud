import tomllib
from pathlib import Path

from xdg_base_dirs import xdg_config_home

from .utils import TEST_DIR

FRONTEND_URL = "https://cloud.moblin.app"


def find_config_toml() -> Path | None:
    paths = [TEST_DIR / "config.toml", xdg_config_home() / "moblin-cloud" / "tests" / "config.toml"]
    for path in paths:
        if path.exists():
            return path
    return None


class Config:
    """The test setup. Without a configuration file everything runs on this machine with the fake Moblin."""

    def __init__(self):
        self.config_toml = find_config_toml()
        if self.config_toml is None:
            self._config = {}
        else:
            self._config = tomllib.loads(self.config_toml.read_text())
        self._validate()

    def general(self) -> dict:
        return self._config.get("general", {})

    def backend_port(self) -> int:
        return self.general().get("backend-port", 8090)

    def remote_control_port(self) -> int:
        return self.general().get("remote-control-port", 2345)

    def headless_browser(self) -> bool:
        return self.general().get("headless-browser", True)

    def twitch(self) -> dict | None:
        return self._config.get("twitch")

    def twitch_sign_in(self) -> str:
        twitch = self.twitch() or {}
        return twitch.get("sign-in", "fake")

    def _validate(self):
        if self.twitch_sign_in() not in ["fake", "real"]:
            raise Exception(f"[twitch] sign-in must be 'fake' or 'real' in '{self.config_toml}'.")
        if self.twitch_sign_in() == "real" and self.twitch() is None:
            raise Exception("Real Twitch sign in needs [twitch] client-id and client-secret.")
