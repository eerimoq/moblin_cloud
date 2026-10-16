import base64
import hashlib
import hmac
import re
import time

import requests
from websockets.exceptions import InvalidStatus
from websockets.sync.client import connect

from ..utils.backend import SESSION_SECRET
from ..utils.config import FRONTEND_URL
from ..utils.context import Context
from ..utils.test_case import TestCase
from ..utils.website import Dashboard

RE_STREAMER_URL = re.compile(r"^ws://localhost:\d+/remote-control/streamer/([A-Za-z0-9_-]{43})$")


def unpadded_base64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def session_issued_at(session: str | None, issued: int) -> str:
    """The same session as if it was issued at another time, signed with the backend's session secret."""
    assert session is not None
    encoded = session.split(".")[0]
    payload = base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4)).decode()
    account, generation, _ = payload.rsplit(":", 2)
    signed = f"{account}:{generation}:{issued}".encode()
    tag = hmac.new(SESSION_SECRET.encode(), signed, hashlib.sha256).digest()
    return f"{unpadded_base64url(signed)}.{unpadded_base64url(tag)}"


def streamer_connects(url: str) -> bool:
    try:
        with connect(url, open_timeout=10) as websocket:
            return "hello" in websocket.recv(timeout=10)
    except InvalidStatus as error:
        if error.response.status_code == 404:
            return False
        raise


class AccountTestCase(TestCase):
    """Tests needing made-up Twitch users, each with its own fresh account."""

    def setup(self):
        self.skip_if_real_twitch_sign_in()

    def assert_signed_out(self, dashboard: Dashboard):
        self.assert_backend_error(401, dashboard.account, "not signed in")


class SignIn(AccountTestCase):
    """Sign in with Twitch from the dashboard and get the account, which has no page yet."""

    def run(self):
        callback_url, browser = self.website.start_sign_in("signin")
        self.assert_true(callback_url.startswith(f"{self.backend.url}/auth/twitch/callback?"))
        self.assert_in("moblin_sign_in_state", browser.cookies)
        dashboard_url = self.website.finish_sign_in(callback_url, browser)
        self.assert_true(dashboard_url.startswith(f"{FRONTEND_URL}/dashboard/#session="))
        account = self.website.dashboard_of(dashboard_url).account()
        self.assert_equal(
            account, {"name": "signin", "login": "signin", "streamerUrl": None, "settings": None}
        )


class SignInWithTwitch(TestCase):
    """Sign in with a real Twitch account in a browser and get the account."""

    def run(self):
        self.skip_if_no_twitch()
        if self.context.config.twitch_sign_in() != "real":
            self.skip_if_not_interactive()
        account = self.sign_in().account()
        self.assert_not_equal(account["login"], "")
        self.assert_not_equal(account["name"], "")


class SignInFromAnotherBrowser(AccountTestCase):
    """Fail to sign in when the callback is opened in another browser than the one that started it, or twice."""

    def run(self):
        callback_url, _ = self.website.start_sign_in("otherbrowser")
        dashboard_url = self.website.finish_sign_in(callback_url, requests.Session())
        self.assert_equal(dashboard_url, f"{FRONTEND_URL}/dashboard/#signInFailed")
        callback_url, browser = self.website.start_sign_in("twice")
        self.website.finish_sign_in(callback_url, browser)
        replaying = requests.Session()
        replaying.headers["cookie"] = f"moblin_sign_in_state={callback_url.split('state=')[1]}"
        dashboard_url = self.website.finish_sign_in(callback_url, replaying)
        self.assert_equal(dashboard_url, f"{FRONTEND_URL}/dashboard/#signInFailed")
        dashboard_url = self.website.finish_sign_in(f"{self.backend.url}/auth/twitch/callback", browser)
        self.assert_equal(dashboard_url, f"{FRONTEND_URL}/dashboard/#signInFailed")


class NotSignedIn(TestCase):
    """Refuse every account request without a valid session."""

    def run(self):
        for session in [None, "garbage", "a.b", session_issued_at("ZmFrZS14OjA6MA.x", int(time.time()))]:
            dashboard = Dashboard(self.backend.url, session)
            self.assert_backend_error(401, dashboard.account, "not signed in")
            self.assert_backend_error(401, dashboard.create_page, "not signed in")
            self.assert_backend_error(401, dashboard.new_streamer_url, "not signed in")
            self.assert_backend_error(401, dashboard.save_settings, "not signed in")
            self.assert_backend_error(401, dashboard.check_paypal, "not signed in")
            self.assert_backend_error(401, dashboard.disconnect_paypal, "not signed in")
            self.assert_backend_error(401, dashboard.sign_out_everywhere, "not signed in")


class ExpiredSession(AccountTestCase):
    """Refuse a session issued more than 30 days ago."""

    def run(self):
        dashboard = self.sign_in("expired")
        dashboard.account()
        old = session_issued_at(dashboard.session, int(time.time()) - 31 * 24 * 60 * 60)
        self.assert_signed_out(Dashboard(self.backend.url, old))


class CreatePage(AccountTestCase):
    """Create the streamer's page on the first visit, and get the same page on later visits."""

    def run(self):
        dashboard = self.sign_in("createpage")
        self.assert_backend_error(404, dashboard.save_settings, "no page")
        self.assert_backend_error(404, dashboard.check_paypal, "no page")
        self.assert_backend_error(404, dashboard.disconnect_paypal, "no page")
        self.assert_backend_error(404, dashboard.new_streamer_url, "no page")
        account = dashboard.create_page()
        self.assert_regex(account["streamerUrl"], RE_STREAMER_URL)
        settings = account["settings"]
        self.assert_equal(
            {
                key: settings[key]
                for key in ["connected", "currency", "paywall", "tips", "spin", "merchantId"]
            },
            {
                "connected": False,
                "currency": "USD",
                "paywall": None,
                "tips": None,
                "spin": None,
                "merchantId": None,
            },
        )
        self.assert_equal(settings["paypal"], self.context.config.paypal() is not None)
        self.assert_equal(settings["paypalSandbox"], self.context.config.paypal() is not None)
        self.assert_equal(dashboard.create_page(), account)
        self.assert_equal(dashboard.account(), account)
        page_id = settings["id"]
        self.assert_equal(self.website.id_of_twitch_login(account["login"]), page_id)
        self.assert_equal(self.website.id_of_twitch_login(account["login"].upper()), page_id)
        profile = self.website.viewer(page_id).profile()
        self.assert_equal(profile["name"], account["name"])
        self.assert_equal(profile["login"], account["login"])

    def assert_regex(self, text: str, regex: re.Pattern):
        self.assert_true(regex.match(text) is not None, f"{text!r} matches {regex.pattern!r}")


class UnknownPage(TestCase):
    """Answer not found for pages and logins nobody has."""

    def run(self):
        self.assert_backend_error(404, lambda: self.website.id_of_twitch_login("nobody_has_this_login"))
        viewer = self.website.viewer("AAAAAAAAAAAAAAAA")
        self.assert_backend_error(404, viewer.profile)
        self.assert_equal(viewer.paywall(), None)
        self.assert_equal(viewer.tips(), None)
        self.assert_equal(viewer.spin(), {"spin": None, "wheel": None})
        self.assert_backend_error(
            404, viewer.queue, "The streamer is offline right now. Try again when they are live."
        )


class NewStreamerUrl(AccountTestCase):
    """Replace the streamer URL, which stops the old one from working but keeps the page."""

    def run(self):
        dashboard = self.sign_in("newstreamerurl")
        account = dashboard.create_page()
        old_url = account["streamerUrl"]
        self.assert_true(streamer_connects(old_url))
        new_account = dashboard.new_streamer_url()
        new_url = new_account["streamerUrl"]
        self.assert_not_equal(new_url, old_url)
        self.assert_equal(new_account["settings"]["id"], account["settings"]["id"])
        self.assert_equal(dashboard.account()["streamerUrl"], new_url)
        self.assert_false(streamer_connects(old_url))
        self.assert_true(streamer_connects(new_url))
        self.assert_false(streamer_connects(f"ws://localhost:{self.backend.port}/remote-control/streamer/x"))


class SignOutEverywhere(AccountTestCase):
    """Sign out everywhere, which ends all sessions, and sign in again."""

    def run(self):
        first = self.sign_in("signout")
        second = self.sign_in("signout")
        page_id = first.create_page()["settings"]["id"]
        first.sign_out_everywhere()
        self.assert_signed_out(first)
        self.assert_signed_out(second)
        third = self.sign_in("signout")
        self.assert_equal(third.account()["settings"]["id"], page_id)


class PageLook(AccountTestCase):
    """Change the look of the page in the dashboard and see it on the page."""

    def run(self):
        dashboard = self.sign_in("pagelook")
        page_id = dashboard.create_page()["settings"]["id"]
        settings = dashboard.save_settings(
            title="  Hello!  ", message="Thanks for watching.", accent="purple"
        )
        self.assert_equal(settings["title"], "Hello!")
        self.assert_equal(settings["message"], "Thanks for watching.")
        self.assert_equal(settings["accent"], "purple")
        profile = self.website.viewer(page_id).profile()
        self.assert_equal(profile["title"], "Hello!")
        self.assert_equal(profile["message"], "Thanks for watching.")
        self.assert_equal(profile["accent"], "purple")
        settings = dashboard.save_settings(title="   ", message="", accent="green")
        self.assert_equal([settings["title"], settings["message"], settings["accent"]], [None, None, None])
        settings = dashboard.save_settings(title=60 * "å", message=300 * "x")
        self.assert_equal(settings["title"], 60 * "å")
        invalid = (
            "the title can be at most 60 characters, the message at most 300 and the accent one of the "
            "supported colors"
        )
        self.assert_backend_error(400, lambda: dashboard.save_settings(title=61 * "x"), invalid)
        self.assert_backend_error(400, lambda: dashboard.save_settings(message=301 * "x"), invalid)
        self.assert_backend_error(400, lambda: dashboard.save_settings(accent="black"), invalid)
        self.assert_equal(dashboard.account()["settings"]["title"], 60 * "å")


class Currency(AccountTestCase):
    """Change the currency of a page without paid features, which needs no PayPal."""

    def run(self):
        dashboard = self.sign_in("currency")
        dashboard.create_page()
        self.assert_equal(dashboard.save_settings(currency=" eur ")["currency"], "EUR")
        self.assert_equal(dashboard.save_settings()["currency"], "EUR")
        self.assert_backend_error(
            400, lambda: dashboard.save_settings(currency="XYZ"), "the currency is not supported"
        )
        self.assert_equal(dashboard.save_settings(currency="USD")["currency"], "USD")


class PaidFeaturesNeedPayPal(AccountTestCase):
    """Refuse to turn song requests, tips or the wheel on before PayPal is connected."""

    def run(self):
        dashboard = self.sign_in("paidneedspaypal")
        page_id = dashboard.create_page()["settings"]["id"]
        if self.context.config.paypal() is None:
            status, text = 503, "payments are unavailable"
        else:
            status, text = 409, "connect PayPal first"
        for changes in [
            {"paywall": {"price": 100}},
            {"tips": {"amounts": [100]}},
            {"spin": {"options": [{"spins": 1, "price": 100}]}},
        ]:
            self.assert_backend_error(
                status, lambda changes=changes: dashboard.save_settings(**changes), text
            )
        viewer = self.website.viewer(page_id)
        self.assert_equal(viewer.paywall(), None)
        self.assert_equal(viewer.tips(), None)
        self.assert_equal(viewer.spin()["spin"], None)


class Cors(TestCase):
    """Only let https://moblin.app call the backend from a browser."""

    def run(self):
        for origin in [FRONTEND_URL, "https://evil.example"]:
            response = requests.options(
                f"{self.backend.url}/account",
                headers={
                    "origin": origin,
                    "access-control-request-method": "GET",
                    "access-control-request-headers": "authorization",
                },
                timeout=10,
            )
            self.assert_equal(response.headers.get("access-control-allow-origin"), FRONTEND_URL)
            response = requests.get(f"{self.backend.url}/streamers", headers={"origin": origin}, timeout=10)
            self.assert_equal(response.headers.get("access-control-allow-origin"), FRONTEND_URL)


def tests(context: Context):
    return [
        SignIn(context),
        SignInWithTwitch(context),
        SignInFromAnotherBrowser(context),
        NotSignedIn(context),
        ExpiredSession(context),
        CreatePage(context),
        UnknownPage(context),
        NewStreamerUrl(context),
        SignOutEverywhere(context),
        PageLook(context),
        Currency(context),
        PaidFeaturesNeedPayPal(context),
        Cors(context),
    ]
