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

    def fake_sign_in(self, login: str) -> "Dashboard":
        callback_url, browser = self.start_sign_in(login)
        return self.dashboard_of(self.finish_sign_in(callback_url, browser))

    def dashboard_of(self, dashboard_url: str) -> "Dashboard":
        session = session_of(dashboard_url)
        if session is None:
            raise Exception(f"Signing in failed, ended at {dashboard_url}.")
        return Dashboard(self.url, session)


def session_of(dashboard_url: str) -> str | None:
    parts = urlsplit(dashboard_url)
    if f"{parts.scheme}://{parts.netloc}{parts.path}" != f"{FRONTEND_URL}/dashboard/":
        return None
    return parse_qs(parts.fragment).get("session", [None])[0]


class Dashboard(Client):
    """A streamer signed in at https://moblin.app/dashboard/."""

    def account(self) -> dict:
        return self.get("/account")

    def create_page(self) -> dict:
        return self.post("/account/page")

    def new_streamer_url(self) -> dict:
        return self.post("/account/streamer-url")

    def save_settings(
        self,
        currency: str | None = None,
        paywall: dict | None = None,
        tips: dict | None = None,
        spin: dict | None = None,
        title: str | None = None,
        message: str | None = None,
        accent: str | None = None,
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
            },
        )

    def update_settings(self, **changes) -> dict:
        """Saves the current settings with some changed, like editing one field in the dashboard."""
        settings = self.account()["settings"]
        fields = ["currency", "paywall", "tips", "spin", "title", "message", "accent"]
        current = {field: settings[field] for field in fields}
        return self.save_settings(**{**current, **changes})

    def connect_paypal(self) -> str:
        return self.post("/account/paypal/connect")["url"]

    def check_paypal(self) -> dict:
        return self.post("/account/paypal/check")

    def disconnect_paypal(self) -> dict:
        return self.post("/account/paypal/disconnect")

    def sign_out_everywhere(self):
        response = self.request("POST", "/account/sign-out")
        if not response.ok:
            raise BackendError(response.status_code, response.text)

    def page_id(self) -> str:
        return self.account()["settings"]["id"]

    def streamer_url(self) -> str:
        return self.account()["streamerUrl"]


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

    def order_song(self, song: str, user: str | None = None) -> str:
        return self.post(self._path("/songs/orders"), {"song": song, "user": user})["orderId"]

    def capture_song(self, order_id: str) -> dict:
        return self.post(self._path(f"/songs/orders/{quote(order_id, safe='')}/capture"))

    def tips(self) -> dict | None:
        return self.get(self._path("/tips"))["tips"]

    def order_tip(self, amount: int, user: str | None = None, message: str | None = None) -> str:
        body = {"amount": amount, "user": user, "message": message}
        return self.post(self._path("/tips/orders"), body)["orderId"]

    def capture_tip(self, order_id: str) -> dict:
        return self.post(self._path(f"/tips/orders/{quote(order_id, safe='')}/capture"))

    def spin(self) -> dict:
        return self.get(self._path("/spin"))

    def order_spin(self, spins: int, user: str | None = None, message: str | None = None) -> str:
        body = {"spins": spins, "user": user, "message": message}
        return self.post(self._path("/spin/orders"), body)["orderId"]

    def capture_spin(self, order_id: str) -> dict:
        return self.post(self._path(f"/spin/orders/{quote(order_id, safe='')}/capture"))
