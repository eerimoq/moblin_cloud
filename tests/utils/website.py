import logging
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
    """Talks to the backend like the website's JavaScript does, from https://moblin.app."""

    def __init__(self, url: str, session: str | None = None):
        self.url = url
        self.session = session

    def request(self, method: str, path: str, json=None, headers: dict | None = None) -> requests.Response:
        headers = {"origin": FRONTEND_URL, **(headers or {})}
        if self.session is not None:
            headers["authorization"] = f"Bearer {self.session}"
        LOGGER.debug("%s %s %s", method, path, json)
        response = requests.request(method, f"{self.url}{path}", json=json, headers=headers, timeout=TIMEOUT)
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


class Website(Client):
    """The public parts of the website: the home page and the streamer list."""

    def streamers(self) -> list[dict]:
        return self.get("/streamers")["streamers"]

    def twitch_live(self) -> dict:
        return self.get("/twitch/live")

    def challenge(self) -> str:
        return self.post("/streamers/live/challenge")["challenge"]

    def went_live(self, body: dict, headers: dict | None = None) -> requests.Response:
        return self.request("POST", "/streamers/live", json=body, headers=headers)

    def viewer(self, page_id: str) -> "Viewer":
        return Viewer(self.url, page_id)

    def id_of_twitch_login(self, login: str) -> str:
        return self.get(f"/pages/twitch/{quote(login, safe='')}")["id"]

    def start_sign_in(self, login: str | None = None) -> tuple[str, requests.Session]:
        """Opens the sign in URL like the dashboard's sign in button, returning where it redirects."""
        browser = requests.Session()
        params = {"login": login} if login is not None else None
        response = browser.get(
            f"{self.url}/auth/twitch/login", params=params, allow_redirects=False, timeout=TIMEOUT
        )
        if response.status_code != 303:
            raise BackendError(response.status_code, response.text)
        return response.headers["location"], browser

    def finish_sign_in(self, callback_url: str, browser: requests.Session) -> str:
        """Follows Twitch's redirect to the callback and returns the dashboard URL it ends at."""
        response = browser.get(callback_url, allow_redirects=False, timeout=TIMEOUT)
        if response.status_code != 303:
            raise BackendError(response.status_code, response.text)
        return response.headers["location"]

    def start_viewer_sign_in(self, page_id: str, login: str | None = None) -> tuple[str, requests.Session]:
        browser = requests.Session()
        params = {"page": page_id, **({"login": login} if login is not None else {})}
        response = browser.get(
            f"{self.url}/auth/twitch/login", params=params, allow_redirects=False, timeout=TIMEOUT
        )
        if response.status_code != 303:
            raise BackendError(response.status_code, response.text)
        return response.headers["location"], browser

    def fake_viewer_sign_in(self, page_id: str, login: str) -> str:
        callback_url, browser = self.start_viewer_sign_in(page_id, login)
        return self.finish_sign_in(callback_url, browser)

    def subscriber(self, page_id: str, login: str) -> "Subscriber":
        page_url = self.fake_viewer_sign_in(page_id, login)
        viewer = viewer_of(page_url)
        if viewer is None:
            raise Exception(f"Signing in as a viewer failed, ended at {page_url}.")
        return Subscriber(self.url, page_id, viewer)

    def fake_sign_in(self, login: str) -> "Dashboard":
        callback_url, browser = self.start_sign_in(login)
        return self.dashboard_of(self.finish_sign_in(callback_url, browser))

    def dashboard_of(self, dashboard_url: str) -> "Dashboard":
        session = session_of(dashboard_url)
        if session is None:
            raise Exception(f"Signing in failed, ended at {dashboard_url}.")
        return Dashboard(self.url, session)


def viewer_of(page_url: str) -> str | None:
    return parse_qs(urlsplit(page_url).fragment).get("viewer", [None])[0]


def session_of(dashboard_url: str) -> str | None:
    parts = urlsplit(dashboard_url)
    if f"{parts.scheme}://{parts.netloc}{parts.path}" != f"{FRONTEND_URL}/dashboard/":
        return None
    return parse_qs(parts.fragment).get("session", [None])[0]


class Dashboard(Client):
    """A streamer signed in at https://moblin.app/dashboard/."""

    def account(self) -> dict:
        return self.get("/account")

    def add_device(self, name: str) -> dict:
        return self.post("/account/devices", {"name": name})

    def choose_devices(self, music: str, wheel: str) -> dict:
        return self.post("/account/devices/choose", {"music": music, "wheel": wheel})

    def new_moblin_cloud_key(self, device: str | None = None) -> dict:
        return self.post(f"/account/devices/{device or self.device_id()}/key")

    def remove_device(self, device: str) -> dict:
        return self.post(f"/account/devices/{device}/remove")

    def save_settings(
        self,
        currency: str | None = None,
        paywall: dict | None = None,
        tips: dict | None = None,
        spin: dict | None = None,
        title: str | None = None,
        message: str | None = None,
        accent: str | None = None,
        font: str | None = None,
        sub_songs: dict | None = None,
        sub_spins: dict | None = None,
        max_song_minutes: int | None = None,
    ) -> dict:
        return self.post(
            "/account/settings",
            {
                "currency": currency,
                "paywall": paywall,
                "tips": tips,
                "spin": spin,
                "title": title,
                "message": message,
                "accent": accent,
                "font": font,
                "subSongs": sub_songs,
                "subSpins": sub_spins,
                "maxSongMinutes": max_song_minutes,
            },
        )

    def update_settings(self, **changes) -> dict:
        """Saves the current settings with some changed, like editing one field in the dashboard."""
        settings = self.account()["settings"]
        fields = [
            "currency",
            "paywall",
            "tips",
            "spin",
            "title",
            "message",
            "accent",
            "font",
            "subSongs",
            "subSpins",
            "maxSongMinutes",
        ]
        names = {"subSongs": "sub_songs", "subSpins": "sub_spins", "maxSongMinutes": "max_song_minutes"}
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

    def test_song(self, song: str) -> dict:
        return self.post("/account/test/song", {"song": song})

    def test_tip(self, message: str | None = None) -> dict:
        return self.post("/account/test/tip", {"message": message})

    def test_spin(self, message: str | None = None) -> dict:
        return self.post("/account/test/spin", {"message": message})

    def admin_stats(self) -> dict:
        return self.get("/admin/stats")

    def enable_moderator(self, moderator: str, enabled: bool) -> dict:
        return self.post(f"/account/moderators/{quote(moderator, safe='')}", {"enabled": enabled})

    def moderate_music(self, page_id: str, command: str, song: str | None = None) -> dict:
        return self.post(f"/account/pages/{quote(page_id, safe='')}/music", {"command": command, "id": song})

    def add_free_song(self, page_id: str, song: str, message: str | None = None) -> dict:
        return self.post(
            f"/account/pages/{quote(page_id, safe='')}/songs", {"song": song, "message": message}
        )

    def moderated_recent(self, page_id: str, kind: str) -> dict:
        return self.get(f"/account/pages/{quote(page_id, safe='')}/recent?kind={kind}")

    def moderated_account(self, page_id: str) -> dict:
        return self.get(f"/account/pages/{quote(page_id, safe='')}")

    def moderated_settings(self, page_id: str, **changes) -> dict:
        """Saves the moderated page's settings with some changed, like the moderator's dashboard does."""
        settings = self.moderated_account(page_id)["settings"]
        names = {"subSongs": "sub_songs", "subSpins": "sub_spins", "maxSongMinutes": "max_song_minutes"}
        body = {
            field: settings[field]
            for field in [
                "currency",
                "paywall",
                "tips",
                "spin",
                "title",
                "message",
                "accent",
                "font",
                "subSongs",
                "subSpins",
                "maxSongMinutes",
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

    def page_id(self) -> str:
        return self.account()["settings"]["id"]

    def device_id(self) -> str:
        return self.account()["devices"][0]["id"]

    def moblin_cloud_key(self) -> str:
        return self.account()["devices"][0]["key"]


class Viewer(Client):
    """A viewer on a streamer's page, https://moblin.app/twitch/<login>."""

    def __init__(self, url: str, page_id: str):
        super().__init__(url)
        self.page_id = page_id

    def _path(self, path: str = "") -> str:
        return f"/pages/{quote(self.page_id, safe='')}{path}"

    def profile(self) -> dict:
        return self.get(self._path())

    def paywall(self) -> dict | None:
        return self.get(self._path("/songs"))["paywall"]

    def queue(self) -> dict:
        return self.get(self._path("/songs/queue"))

    def order_song(self, song: str, user: str | None = None, provider: str | None = None) -> str:
        body = {"song": song, "user": user, "provider": provider}
        return self.post(self._path("/songs/orders"), body)["orderId"]

    def capture_song(self, order_id: str) -> dict:
        return self.post(self._path(f"/songs/orders/{quote(order_id, safe='')}/capture"))

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


class Subscriber(Viewer):
    def __init__(self, url: str, page_id: str, token: str):
        super().__init__(url, page_id)
        self.token = token
        self.session = token

    def songs(self) -> dict:
        return self.get(self._path("/songs/subscriber"))

    def request_song(self, song: str, message: str | None = None) -> dict:
        return self.post(self._path("/songs/subscriber"), {"song": song, "message": message})

    def spins(self) -> dict:
        return self.get(self._path("/spin/subscriber"))

    def spin_wheel(self, message: str | None = None) -> dict:
        return self.post(self._path("/spin/subscriber"), {"message": message})
