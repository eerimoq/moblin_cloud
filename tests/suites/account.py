import base64
import hashlib
import hmac
import re
import time
from functools import partial

import requests
from websockets.exceptions import InvalidStatus
from websockets.sync.client import connect

from ..utils.backend import SESSION_SECRET
from ..utils.config import FRONTEND_URL
from ..utils.context import Context
from ..utils.test_case import TestCase
from ..utils.website import Dashboard

RE_MOBLIN_CLOUD_KEY = re.compile(r"^[A-Za-z0-9_-]{43}$")


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


def streamer_connects(port: int, key: str) -> bool:
    try:
        with connect(f"ws://localhost:{port}/remote-control/streamer/{key}", open_timeout=10) as websocket:
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


class AccountSignIn(AccountTestCase):
    """Sign in with Twitch from the dashboard and get the account, which has no page yet."""

    def run(self):
        callback_url, browser = self.website.start_sign_in("signin")
        self.assert_true(callback_url.startswith(f"{self.backend.url}/auth/twitch/callback?"))
        self.assert_in("moblin_sign_in_state", browser.cookies)
        dashboard_url = self.website.finish_sign_in(callback_url, browser)
        self.assert_true(dashboard_url.startswith(f"{FRONTEND_URL}/dashboard/#session="))
        account = self.website.dashboard_of(dashboard_url).account()
        self.assert_equal(
            account,
            {
                "name": "signin",
                "login": "signin",
                "devices": [],
                "musicDevice": None,
                "wheelDevice": None,
                "settings": None,
                "admin": False,
                "pro": False,
            },
        )


class AccountSignInWithTwitch(TestCase):
    """Sign in with a real Twitch account in a browser and get the account."""

    def run(self):
        self.skip_if_no_twitch()
        if self.context.config.twitch_sign_in() != "real":
            self.skip_if_not_interactive()
        account = self.sign_in().account()
        self.assert_not_equal(account["login"], "")
        self.assert_not_equal(account["name"], "")


class AccountSignInFromAnotherBrowser(AccountTestCase):
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


class AccountNotSignedIn(TestCase):
    """Refuse every account request without a valid session."""

    def run(self):
        for session in [None, "garbage", "a.b", session_issued_at("ZmFrZS14OjA6MA.x", int(time.time()))]:
            dashboard = Dashboard(self.backend.url, session)
            self.assert_backend_error(401, dashboard.account, "not signed in")
            self.assert_backend_error(401, dashboard.create_page, "not signed in")
            self.assert_backend_error(401, partial(dashboard.new_moblin_cloud_key, "d"), "not signed in")
            self.assert_backend_error(401, partial(dashboard.add_device, "Moblin"), "not signed in")
            self.assert_backend_error(401, partial(dashboard.remove_device, "d"), "not signed in")
            self.assert_backend_error(401, dashboard.save_settings, "not signed in")
            self.assert_backend_error(401, dashboard.check_paypal, "not signed in")
            self.assert_backend_error(401, dashboard.disconnect_paypal, "not signed in")
            self.assert_backend_error(401, dashboard.sign_out_everywhere, "not signed in")


class AccountExpiredSession(AccountTestCase):
    """Refuse a session issued more than 30 days ago."""

    def run(self):
        dashboard = self.sign_in("expired")
        dashboard.account()
        old = session_issued_at(dashboard.session, int(time.time()) - 31 * 24 * 60 * 60)
        self.assert_signed_out(Dashboard(self.backend.url, old))


class AccountCreatePage(AccountTestCase):
    """Create the streamer's page on the first visit, and get the same page on later visits."""

    def run(self):
        dashboard = self.sign_in("createpage")
        self.assert_backend_error(404, dashboard.save_settings, "no page")
        self.assert_backend_error(404, dashboard.check_paypal, "no page")
        self.assert_backend_error(404, dashboard.disconnect_paypal, "no page")
        self.assert_backend_error(404, lambda: dashboard.add_device("Moblin"), "no page")
        account = dashboard.create_page()
        self.assert_equal(len(account["devices"]), 1)
        self.assert_regex(account["devices"][0]["key"], RE_MOBLIN_CLOUD_KEY)
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


class AccountUnknownPage(TestCase):
    """Answer not found for pages and logins nobody has."""

    def run(self):
        self.assert_backend_error(404, lambda: self.website.id_of_twitch_login("nobody_has_this_login"))
        viewer = self.website.viewer("AAAAAAAAAAAAAAAA")
        self.assert_backend_error(404, viewer.profile)
        self.assert_equal(viewer.paywall(), None)
        self.assert_equal(viewer.tips(), None)
        self.assert_equal(viewer.spin(), {"spin": None, "wheel": None, "connected": False})
        self.assert_backend_error(
            404, viewer.queue, "The streamer is offline right now. Try again when they are live."
        )


class AccountNewMoblinCloudKey(AccountTestCase):
    """Replace the Moblin Cloud key, which stops the old one from working but keeps the page."""

    def run(self):
        dashboard = self.sign_in("newcloudkey")
        account = dashboard.create_page()
        port = self.backend.port
        old_key = account["devices"][0]["key"]
        self.assert_true(streamer_connects(port, old_key))
        new_account = dashboard.new_moblin_cloud_key()
        new_key = new_account["devices"][0]["key"]
        self.assert_not_equal(new_key, old_key)
        self.assert_equal(new_account["settings"]["id"], account["settings"]["id"])
        self.assert_equal(dashboard.moblin_cloud_key(), new_key)
        self.assert_false(streamer_connects(port, old_key))
        self.assert_true(streamer_connects(port, new_key))
        self.assert_false(streamer_connects(port, "x"))


class AccountDevices(AccountTestCase):
    """Add up to 5 Moblin devices, each with its own Moblin Cloud key, pick devices and remove them."""

    def run(self):
        dashboard = self.sign_in("devices")
        account = dashboard.create_page()
        first = account["devices"][0]["id"]
        self.assert_backend_error(
            400, lambda: dashboard.add_device(" "), "the name must be 1 to 30 characters"
        )
        for number in range(2, 6):
            account = dashboard.add_device(f"Phone {number}")
        self.assert_equal(
            [device["name"] for device in account["devices"]][1:],
            ["Phone 2", "Phone 3", "Phone 4", "Phone 5"],
        )
        self.assert_backend_error(409, lambda: dashboard.add_device("Phone 6"), "at most 5 Moblin devices")
        keys = [device["key"] for device in account["devices"]]
        self.assert_equal(len(set(keys)), 5)
        port = self.backend.port
        self.assert_true(all(streamer_connects(port, key) for key in keys))
        second = account["devices"][1]["id"]
        self.assert_equal((account["musicDevice"], account["wheelDevice"]), (first, first))
        self.assert_backend_error(404, lambda: dashboard.choose_devices(second, "x"), "no such device")
        account = dashboard.choose_devices(second, first)
        self.assert_equal((account["musicDevice"], account["wheelDevice"]), (second, first))
        account = dashboard.remove_device(second)
        self.assert_equal(len(account["devices"]), 4)
        self.assert_equal(account["musicDevice"], first)
        self.assert_false(streamer_connects(port, keys[1]))
        self.assert_backend_error(404, lambda: dashboard.remove_device(second), "no such device")


class AccountSignOutEverywhere(AccountTestCase):
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


class AccountPageLook(AccountTestCase):
    """Change the look of the page in the dashboard and see it on the page."""

    def run(self):
        dashboard = self.sign_in("pagelook")
        page_id = dashboard.create_page()["settings"]["id"]
        settings = dashboard.save_settings(
            title="  Hello!  ", message="Thanks for watching.", accent="purple", font="serif"
        )
        self.assert_equal(settings["title"], "Hello!")
        self.assert_equal(settings["message"], "Thanks for watching.")
        self.assert_equal(settings["accent"], "purple")
        self.assert_equal(settings["font"], "serif")
        profile = self.website.viewer(page_id).profile()
        self.assert_equal(profile["title"], "Hello!")
        self.assert_equal(profile["message"], "Thanks for watching.")
        self.assert_equal(profile["accent"], "purple")
        self.assert_equal(profile["font"], "serif")
        settings = dashboard.save_settings(title="   ", message="", accent="green", font="rounded")
        self.assert_equal(
            [settings["title"], settings["message"], settings["accent"], settings["font"]],
            [None, None, None, None],
        )
        settings = dashboard.save_settings(title=60 * "å", message=300 * "x")
        self.assert_equal(settings["title"], 60 * "å")
        invalid = (
            "the title can be at most 60 characters, the message at most 300 and the accent and font "
            "among the supported ones"
        )
        self.assert_backend_error(400, lambda: dashboard.save_settings(title=61 * "x"), invalid)
        self.assert_backend_error(400, lambda: dashboard.save_settings(message=301 * "x"), invalid)
        self.assert_backend_error(400, lambda: dashboard.save_settings(accent="black"), invalid)
        self.assert_backend_error(400, lambda: dashboard.save_settings(font="comic"), invalid)
        self.assert_equal(dashboard.account()["settings"]["title"], 60 * "å")
        normal = self.sign_in("normallook")
        normal.create_page()
        self.assert_backend_error(
            403, lambda: normal.save_settings(font="serif"), "fonts are only for pro streamers"
        )
        self.assert_equal(normal.save_settings(accent="purple", font="rounded")["font"], None)


class AccountCurrency(AccountTestCase):
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


class AccountPaidFeaturesNeedPayments(AccountTestCase):
    """Refuse to turn song requests, tips or the wheel on before PayPal or Stripe is connected."""

    def run(self):
        dashboard = self.sign_in("paidneedspaypal")
        page_id = dashboard.create_page()["settings"]["id"]
        if self.context.config.paypal() is None and self.context.config.stripe() is None:
            status, text = 503, "payments are unavailable"
        else:
            status, text = 409, "connect PayPal or Stripe first"
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


class AccountTestDeliveriesNeedMoblin(AccountTestCase):
    """Refuse the dashboard's free test song and wheel spin without a page or a connected Moblin, and queue the test tip."""

    def run(self):
        dashboard = self.sign_in("testneedsmoblin")
        empty = {"items": [], "more": False}
        for kind in ["song", "tip", "spin"]:
            self.assert_equal(dashboard.recent(kind), empty)
        self.assert_backend_error(404, dashboard.test_tip, "no page")
        dashboard.create_page()
        song = "https://music.apple.com/us/album/take-on-me/380907762?i=380907765"
        self.assert_backend_error(409, lambda: dashboard.test_song(song), "Connect Moblin first.")
        self.assert_backend_error(409, dashboard.test_spin, "Connect Moblin first.")
        dashboard.test_tip()
        for kind in ["song", "tip", "spin"]:
            self.assert_equal(dashboard.recent(kind), empty)


class AccountAdmin(AccountTestCase):
    """Show the operator's statistics only to admins."""

    def run(self):
        streamer = self.sign_in("notadmin")
        streamer.create_page()
        self.assert_false(streamer.account()["admin"])
        self.assert_backend_error(403, streamer.admin_stats, "not an admin")
        self.assert_backend_error(401, Dashboard(self.backend.url, None).admin_stats, "not signed in")
        admin = self.sign_in("admin")
        self.assert_true(admin.account()["admin"])
        stats = admin.admin_stats()
        self.assert_equal(sorted(stats), ["accounts", "counters", "moblin", "pending"])
        self.assert_equal(sorted(stats["accounts"]), ["images", "paypal", "stripe", "total", "withPage"])
        self.assert_true(stats["accounts"]["total"] >= 2)
        self.assert_true(stats["accounts"]["withPage"] >= 1)
        self.assert_in("connectedDevices", stats["moblin"])
        self.assert_equal(sorted(stats["pending"]), ["deliveries", "orders"])
        for counter in stats["counters"]:
            self.assert_equal(sorted(counter), ["name", "today", "total", "week"])
            self.assert_true(counter["today"] <= counter["week"] <= counter["total"])


class AccountCors(TestCase):
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
        AccountSignIn(context),
        AccountSignInWithTwitch(context),
        AccountSignInFromAnotherBrowser(context),
        AccountNotSignedIn(context),
        AccountExpiredSession(context),
        AccountCreatePage(context),
        AccountUnknownPage(context),
        AccountNewMoblinCloudKey(context),
        AccountDevices(context),
        AccountSignOutEverywhere(context),
        AccountPageLook(context),
        AccountCurrency(context),
        AccountPaidFeaturesNeedPayments(context),
        AccountTestDeliveriesNeedMoblin(context),
        AccountAdmin(context),
        AccountCors(context),
    ]
