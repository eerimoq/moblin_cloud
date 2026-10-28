import json as jsonlib
import logging
import time
from urllib.parse import parse_qs
from urllib.parse import quote
from urllib.parse import urlsplit

import requests

from .config import FRONTEND_URL

LOGGER = logging.getLogger(__name__)
TIMEOUT = 60


class BackendError(Exception):
    def __init__(self, status: int, text: str):
        super().__init__(f"{status}: {text}")
        self.status = status
        self.text = text


class Client:
    """Talks to the backend like the frontend's JavaScript does, from https://cloud.moblin.app."""

    def __init__(self, url: str, session: str | None = None):
        self.url = url
        self.session = session

    def request(
        self,
        method: str,
        path: str,
        json=None,
        headers: dict | None = None,
        stream: bool = False,
        data: bytes | str | None = None,
    ) -> requests.Response:
        headers = {"origin": FRONTEND_URL, **(headers or {})}
        if self.session is not None:
            headers["authorization"] = f"Bearer {self.session}"
        LOGGER.debug("%s %s %s", method, path, json)
        response = requests.request(
            method,
            f"{self.url}{path}",
            json=json,
            data=data,
            headers=headers,
            timeout=TIMEOUT,
            stream=stream,
        )
        if not stream:
            LOGGER.debug("%s %s -> %s %s", method, path, response.status_code, response.text)
        return response

    def call(self, method: str, path: str, json=None):
        response = self.request(method, path, json)
        if not response.ok:
            raise BackendError(response.status_code, response.text)
        return response.json()

    def get(self, path: str):
        return self.call("GET", path)

    def post(self, path: str, json=None):
        return self.call("POST", path, json)


class Frontend(Client):
    """The public parts of the frontend: the streamer pages and signing in."""

    def viewer(self, page_id: str) -> "Viewer":
        return Viewer(self.url, page_id)

    def id_of_twitch_login(self, login: str) -> str:
        return self.get(f"/pages/twitch/{quote(login, safe='')}")["id"]

    def start_sign_in(self, login: str, platform: str = "twitch") -> tuple[str, requests.Session]:
        """Opens the sign in URL like the dashboard's sign in button and signs in as the made-up user
        on the fake Twitch or Kick it redirects to, returning the callback URL the fake redirects back to."""
        return self._start_sign_in({}, login, platform)

    def _start_sign_in(
        self, params: dict[str, str], login: str, platform: str = "twitch"
    ) -> tuple[str, requests.Session]:
        browser = requests.Session()
        response = browser.get(
            f"{self.url}/auth/{platform}/login", params=params, allow_redirects=False, timeout=TIMEOUT
        )
        if response.status_code != 303:
            raise BackendError(response.status_code, response.text)
        response = browser.get(
            response.headers["location"], params={"login": login}, allow_redirects=False, timeout=TIMEOUT
        )
        if response.status_code != 302:
            raise Exception(f"The fake Twitch answered {response.status_code}: {response.text}")
        return response.headers["location"], browser

    def finish_sign_in(self, callback_url: str, browser: requests.Session) -> str:
        """Follows Twitch's redirect to the callback and returns the dashboard URL it ends at."""
        response = browser.get(callback_url, allow_redirects=False, timeout=TIMEOUT)
        if response.status_code != 303:
            raise BackendError(response.status_code, response.text)
        return response.headers["location"]

    def start_viewer_sign_in(
        self, page_id: str, login: str = "viewer", platform: str = "twitch"
    ) -> tuple[str, requests.Session]:
        return self._start_sign_in({"page": page_id}, login, platform)

    def fake_viewer_sign_in(self, page_id: str, login: str, platform: str = "twitch") -> str:
        callback_url, browser = self.start_viewer_sign_in(page_id, login, platform)
        return self.finish_sign_in(callback_url, browser)

    def subscriber(self, page_id: str, login: str, platform: str = "twitch") -> "Subscriber":
        page_url = self.fake_viewer_sign_in(page_id, login, platform)
        viewer = viewer_of(page_url)
        if viewer is None:
            raise Exception(f"Signing in as a viewer failed, ended at {page_url}.")
        return Subscriber(self.url, page_id, viewer)

    def fake_sign_in(self, login: str, platform: str = "twitch") -> "Dashboard":
        callback_url, browser = self.start_sign_in(login, platform)
        return self.dashboard_of(self.finish_sign_in(callback_url, browser))

    def fake_link(self, platform: str, login: str) -> str:
        """Runs the link flow of the Account page's Link button as the made-up user, returning where
        the backend's callback redirects to, /account/#link=<token> or /account/#linkFailed."""
        callback_url, browser = self._start_sign_in({"link": "1"}, login, platform)
        return self.finish_sign_in(callback_url, browser)

    def link_token(self, platform: str, login: str) -> str:
        account_url = self.fake_link(platform, login)
        token = link_of(account_url)
        if token is None:
            raise Exception(f"Linking failed, ended at {account_url}.")
        return token

    def dashboard_of(self, dashboard_url: str) -> "Dashboard":
        session = session_of(dashboard_url)
        if session is None:
            raise Exception(f"Signing in failed, ended at {dashboard_url}.")
        return Dashboard(self.url, session)


def viewer_of(page_url: str) -> str | None:
    return parse_qs(urlsplit(page_url).fragment).get("viewer", [None])[0]


def link_of(account_url: str) -> str | None:
    parts = urlsplit(account_url)
    if f"{parts.scheme}://{parts.netloc}{parts.path}" != f"{FRONTEND_URL}/account/":
        return None
    return parse_qs(parts.fragment).get("link", [None])[0]


def session_of(dashboard_url: str) -> str | None:
    parts = urlsplit(dashboard_url)
    if f"{parts.scheme}://{parts.netloc}{parts.path}" != f"{FRONTEND_URL}/dashboard/":
        return None
    return parse_qs(parts.fragment).get("session", [None])[0]


def follow_onboarding(url: str) -> str:
    """Opens a payment provider's onboarding link, which returns to the dashboard when done."""
    response = requests.get(url, allow_redirects=False, timeout=30)
    location = response.headers.get("location", "")
    if response.status_code != 302 or not location.startswith(f"{FRONTEND_URL}/dashboard/"):
        raise Exception(f"Onboarding at {url} answered {response.status_code} to {location!r}.")
    return location


class Dashboard(Client):
    """A streamer signed in at https://cloud.moblin.app/dashboard/."""

    def account(self) -> dict:
        return self.get("/account")

    def new_moblin_cloud_key(self) -> dict:
        return self.post("/account/key")

    def rename_device(self, device: str, name: str) -> dict:
        return self.post(f"/account/devices/{device}/name", {"name": name})

    def remove_device(self, device: str) -> dict:
        return self.post(f"/account/devices/{device}/remove")

    def save_settings(
        self,
        currency: str | None = None,
        tips: dict | None = None,
        spin: dict | None = None,
        message: str | None = None,
        accent: str | None = None,
        font: str | None = None,
        sub_spins: dict | None = None,
    ) -> dict:
        return self.post(
            "/account/settings",
            {
                "currency": currency,
                "tips": tips,
                "spin": spin,
                "message": message,
                "accent": accent,
                "font": font,
                "subSpins": sub_spins,
            },
        )

    def update_settings(self, **changes) -> dict:
        """Saves the current settings with some changed, like editing one field in the dashboard."""
        settings = self.account()["settings"]
        fields = [
            "currency",
            "tips",
            "spin",
            "message",
            "accent",
            "font",
            "subSpins",
        ]
        names = {"subSpins": "sub_spins"}
        current = {names.get(field, field): settings[field] for field in fields}
        return self.save_settings(**{**current, **changes})

    def connect_paypal(self) -> str:
        return self.post("/account/paypal/connect")["url"]

    def check_paypal(self) -> dict:
        return self.post("/account/paypal/check")

    def disconnect_paypal(self) -> dict:
        return self.post("/account/paypal/disconnect")

    def connect_stripe(self, country: str | None = "US") -> str:
        return self.post("/account/stripe/connect", {"country": country})["url"]

    def check_stripe(self) -> dict:
        return self.post("/account/stripe/check")

    def disconnect_stripe(self) -> dict:
        return self.post("/account/stripe/disconnect")

    def recent(self, kind: str) -> dict:
        return self.get(f"/account/recent?kind={kind}")

    def resend(self, order_id: str) -> dict:
        return self.post(f"/account/recent/{quote(order_id, safe='')}/resend")

    def test_tip(self, message: str | None = None) -> dict:
        return self.post("/account/test/tip", {"message": message})

    def test_spin(self, message: str | None = None) -> dict:
        return self.post("/account/test/spin", {"message": message})

    def admin_stats(self) -> dict:
        return self.get("/admin/stats")

    def add_moderator(self, platform: str, username: str) -> dict:
        return self.post(f"/account/moderators/{platform}", {"username": username})

    def remove_moderator(self, platform: str, moderator: str) -> dict:
        return self.post(f"/account/moderators/{platform}/{quote(moderator, safe='')}/remove")

    def moderated_recent(self, page_id: str, kind: str) -> dict:
        return self.get(f"/account/pages/{quote(page_id, safe='')}/recent?kind={kind}")

    def moderated_account(self, page_id: str) -> dict:
        return self.get(f"/account/pages/{quote(page_id, safe='')}")

    def device_events(self, page_id: str, device: str) -> "DeviceEvents":
        path = f"/account/pages/{quote(page_id, safe='')}/devices/{quote(device, safe='')}/events"
        response = self.request("GET", path, headers={"accept": "text/event-stream"}, stream=True)
        if not response.ok:
            raise BackendError(response.status_code, response.text)
        return DeviceEvents(response)

    def device_preview(self, page_id: str, device: str) -> "DeviceEvents":
        path = f"/account/pages/{quote(page_id, safe='')}/devices/{quote(device, safe='')}/preview"
        response = self.request("GET", path, headers={"accept": "text/event-stream"}, stream=True)
        if not response.ok:
            raise BackendError(response.status_code, response.text)
        return DeviceEvents(response)

    def control_device(self, page_id: str, device: str, control: dict) -> dict:
        return self.post(
            f"/account/pages/{quote(page_id, safe='')}/devices/{quote(device, safe='')}/control", control
        )

    def moderated_settings(self, page_id: str, **changes) -> dict:
        """Saves the moderated page's settings with some changed, like the moderator's dashboard does."""
        settings = self.moderated_account(page_id)["settings"]
        names = {"subSpins": "sub_spins"}
        body = {
            field: settings[field]
            for field in [
                "currency",
                "tips",
                "spin",
                "message",
                "accent",
                "font",
                "subSpins",
            ]
        }
        for field in body:
            if names.get(field, field) in changes:
                body[field] = changes[names.get(field, field)]
        return self.post(f"/account/pages/{quote(page_id, safe='')}/settings", body)

    def moderated_test_tip(self, page_id: str, message: str | None = None) -> dict:
        return self.post(f"/account/pages/{quote(page_id, safe='')}/test/tip", {"message": message})

    def moderated_test_spin(self, page_id: str, message: str | None = None) -> dict:
        return self.post(f"/account/pages/{quote(page_id, safe='')}/test/spin", {"message": message})

    def moderated_remove_image(self, page_id: str) -> dict:
        return self.post(f"/account/pages/{quote(page_id, safe='')}/image/remove")

    def sign_out_everywhere(self):
        response = self.request("POST", "/account/sign-out")
        if not response.ok:
            raise BackendError(response.status_code, response.text)

    def delete_account(self):
        response = self.request("POST", "/account/delete")
        if not response.ok:
            raise BackendError(response.status_code, response.text)

    def link(self, token: str) -> dict:
        return self.post("/account/link", {"token": token})

    def unlink(self, platform: str) -> dict:
        return self.post(f"/account/unlink/{platform}")

    def change_settings(self, **changes) -> dict:
        """Saves the current settings, effects included, with some changed, by their names in the API."""
        settings = self.account()["settings"]
        return self.post("/account/settings", {**settings_body(settings), **changes})

    def test_effect(self, effect: str | None = None, message: str | None = None) -> dict:
        return self.post("/account/test/effect", {"effect": effect, "message": message})

    def set_moderator_preview(self, platform: str, moderator: str, on: bool) -> dict:
        return self.post(f"/account/moderators/{platform}/{quote(moderator, safe='')}/preview", {"on": on})

    def upload_image(self, data: bytes, page_id: str | None = None) -> dict:
        path = "/account/image" if page_id is None else f"/account/pages/{quote(page_id, safe='')}/image"
        response = self.request("POST", path, data=data, headers={"content-type": "application/octet-stream"})
        if not response.ok:
            raise BackendError(response.status_code, response.text)
        return response.json()

    def remove_image(self) -> dict:
        return self.post("/account/image/remove")

    def moderated_resend(self, page_id: str, order_id: str) -> dict:
        return self.post(f"/account/pages/{quote(page_id, safe='')}/recent/{quote(order_id, safe='')}/resend")

    def moderated_test_effect(self, page_id: str, effect: str | None = None) -> dict:
        return self.post(f"/account/pages/{quote(page_id, safe='')}/test/effect", {"effect": effect})

    def page_id(self) -> str:
        return self.account()["settings"]["id"]

    def moblin_cloud_key(self) -> str:
        return self.account()["key"]


SETTINGS_FIELDS = [
    "currency",
    "tips",
    "spin",
    "effects",
    "message",
    "accent",
    "font",
    "subSpins",
    "subEffects",
]


def settings_body(settings: dict) -> dict:
    return {field: settings[field] for field in SETTINGS_FIELDS}


class Viewer(Client):
    """A viewer on a streamer's page, https://cloud.moblin.app/twitch/<login>."""

    def __init__(self, url: str, page_id: str):
        super().__init__(url)
        self.page_id = page_id

    def _path(self, path: str = "") -> str:
        return f"/pages/{quote(self.page_id, safe='')}{path}"

    def profile(self) -> dict:
        return self.get(self._path())

    def tips(self) -> dict | None:
        return self.get(self._path("/tips"))["tips"]

    def tip_order(
        self, amount: int, user: str | None = None, message: str | None = None, provider: str | None = None
    ) -> dict:
        body = {"amount": amount, "user": user, "message": message, "provider": provider}
        return self.post(self._path("/tips/orders"), body)

    def order_tip(
        self, amount: int, user: str | None = None, message: str | None = None, provider: str | None = None
    ) -> str:
        return self.tip_order(amount, user, message, provider)["orderId"]

    def capture_tip(self, order_id: str) -> dict:
        return self.post(self._path(f"/tips/orders/{quote(order_id, safe='')}/capture"))

    def spin(self) -> dict:
        return self.get(self._path("/spin"))

    def order_spin(
        self, spins: int, user: str | None = None, message: str | None = None, provider: str | None = None
    ) -> str:
        body = {"spins": spins, "user": user, "message": message, "provider": provider}
        return self.post(self._path("/spin/orders"), body)["orderId"]

    def capture_spin(self, order_id: str) -> dict:
        return self.post(self._path(f"/spin/orders/{quote(order_id, safe='')}/capture"))

    def effects(self) -> dict:
        return self.get(self._path("/effects"))

    def order_effect(
        self,
        effect: str,
        price: int,
        user: str | None = None,
        message: str | None = None,
        provider: str | None = None,
    ) -> str:
        body = {"effect": effect, "price": price, "user": user, "message": message, "provider": provider}
        return self.post(self._path("/effects/orders"), body)["orderId"]

    def capture_effect(self, order_id: str) -> dict:
        return self.post(self._path(f"/effects/orders/{quote(order_id, safe='')}/capture"))


class Subscriber(Viewer):
    def __init__(self, url: str, page_id: str, token: str):
        super().__init__(url, page_id)
        self.token = token
        self.session = token

    def spins(self) -> dict:
        return self.get(self._path("/spin/subscriber"))

    def spin_wheel(self, message: str | None = None) -> dict:
        return self.post(self._path("/spin/subscriber"), {"message": message})

    def effect_allowance(self) -> dict:
        return self.get(self._path("/effects/subscriber"))

    def play_effect(self, effect: str, message: str | None = None) -> dict:
        return self.post(self._path("/effects/subscriber"), {"effect": effect, "message": message})


class DeviceEvents:
    """The server-sent events of a device page, read one at a time."""

    def __init__(self, response: requests.Response):
        self._response = response
        self._lines = response.iter_lines(decode_unicode=True)

    def next(self, timeout: float = 10) -> tuple[str, dict]:
        """The next event with data as (name, data), skipping keep alive comments."""
        deadline = time.monotonic() + timeout
        name = "message"
        data: list[str] = []
        for raw in self._lines:
            line = raw if isinstance(raw, str) else raw.decode()
            if time.monotonic() > deadline:
                raise TimeoutError("No event in time.")
            if line.startswith("event:"):
                name = line[6:].strip()
            elif line.startswith("data:"):
                data.append(line[5:].strip())
            elif line == "":
                if data:
                    return name, jsonlib.loads("\n".join(data))
                name = "message"
        raise EOFError("The event stream ended.")

    def wait_for(self, name: str, timeout: float = 10) -> dict:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            event, data = self.next(timeout)
            if event == name:
                return data
        raise TimeoutError(f"No {name} event in time.")

    def close(self):
        self._response.close()
