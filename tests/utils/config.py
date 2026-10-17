import tomllib
from enum import StrEnum
from pathlib import Path

from xdg_base_dirs import xdg_config_home

from .utils import TEST_DIR

FRONTEND_URL = "https://moblin.app"
REMOTE_CONTROL_PASSWORD = "1234"
WEB_REMOTE_CONTROL_PORT = 1180


class Capability(StrEnum):
    APPLE_MUSIC = "apple-music"


def find_config_toml() -> Path | None:
    paths = [TEST_DIR / "config.toml", xdg_config_home() / "moblin-website" / "tests" / "config.toml"]
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

    def tester_ip_address(self) -> str:
        return self.general().get("tester-ip-address", "127.0.0.1")

    def backend_port(self) -> int:
        return self.general().get("backend-port", 8090)

    def remote_control_port(self) -> int:
        return self.general().get("remote-control-port", 2345)

    def headless_browser(self) -> bool:
        return self.general().get("headless-browser", True)

    def allow_unattested(self) -> bool:
        return self.general().get("allow-unattested", True)

    def app_attest_environment(self) -> str:
        return self.general().get("app-attest-environment", "development")

    def app_id(self) -> str | None:
        return self.general().get("app-id")

    def moblin_ip_address(self, device_name: str) -> str:
        return self._device(device_name)["moblin-ip-address"]

    def capabilities(self, device_name: str) -> list[str]:
        return self._device(device_name).get("capabilities", [])

    def twitch(self) -> dict | None:
        return self._config.get("twitch")

    def twitch_sign_in(self) -> str:
        twitch = self.twitch() or {}
        return twitch.get("sign-in", "fake")

    def paypal(self) -> dict | None:
        return self._config.get("paypal")

    def stripe(self) -> dict | None:
        return self._config.get("stripe")

    def _device(self, device_name: str) -> dict:
        device = self._config.get("device", {}).get(device_name)
        if device is None:
            raise Exception(f"Device '{device_name}' not found in the configuration file {self.config_toml}.")
        return device

    def _validate(self):
        for name, device in self._config.get("device", {}).items():
            for capability in device.get("capabilities", []):
                if capability not in list(Capability):
                    allowed = ", ".join(f"'{item}'" for item in Capability)
                    raise Exception(
                        f"Unknown capability '{capability}' for device '{name}' in "
                        f"'{self.config_toml}'. Allowed capabilities are {allowed}."
                    )
        if self.twitch_sign_in() not in ["fake", "real"]:
            raise Exception(f"[twitch] sign-in must be 'fake' or 'real' in '{self.config_toml}'.")
        if self.twitch_sign_in() == "real" and self.twitch() is None:
            raise Exception("Real Twitch sign in needs [twitch] client-id and client-secret.")
        paypal = self.paypal()
        if paypal is not None:
            for key in [
                "client-id",
                "client-secret",
                "partner-id",
                "seller-email",
                "seller-password",
            ]:
                if key not in paypal:
                    raise Exception(f"[paypal] {key} missing in '{self.config_toml}'.")
        stripe = self.stripe()
        if stripe is not None:
            for key in ["secret-key", "publishable-key"]:
                if key not in stripe:
                    raise Exception(f"[stripe] {key} missing in '{self.config_toml}'.")
            if not stripe["secret-key"].startswith("sk_test_"):
                raise Exception(f"[stripe] secret-key must be a test mode key in '{self.config_toml}'.")
