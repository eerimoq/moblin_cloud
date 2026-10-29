import base64
import hashlib
import hmac
import json
import re
import time

import requests
from websockets.exceptions import ConnectionClosed
from websockets.exceptions import InvalidStatus
from websockets.sync.client import ClientConnection
from websockets.sync.client import connect

from ..utils.backend import SESSION_SECRET
from ..utils.config import FRONTEND_URL
from ..utils.context import Context
from ..utils.frontend import Dashboard
from ..utils.test_case import TestCase

RE_MOBLIN_CLOUD_KEY = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")


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


def identified(port: int, key: str, device_id: str) -> ClientConnection | None:
    websocket = connect(f"ws://localhost:{port}/remote-control/streamer/{key}", open_timeout=10)
    try:
        websocket.recv(timeout=10)
        websocket.send(json.dumps({"identify": {"streamerId": device_id, "authentication": "x"}}))
        if json.loads(websocket.recv(timeout=10)) == {"identified": {"result": {"ok": {}}}}:
            return websocket
    except ConnectionClosed:
        pass
    websocket.close()
    return None


class AccountTestCase(TestCase):
    """Tests needing made-up Twitch users, each with its own fresh account."""

    def setup(self):
        self.skip_if_real_twitch_sign_in()

    def assert_signed_out(self, dashboard: Dashboard):
        self.assert_backend_error(401, dashboard.account, "not signed in")


class AccountSignIn(AccountTestCase):
    """Sign in with Twitch from the dashboard and get the account with its page."""

    def run(self):
        callback_url, browser = self.frontend.start_sign_in("signin")
        self.assert_true(callback_url.startswith(f"{self.backend.url}/auth/twitch/callback?"))
        self.assert_in("moblin_sign_in_state", browser.cookies)
        dashboard_url = self.frontend.finish_sign_in(callback_url, browser)
        self.assert_true(dashboard_url.startswith(f"{FRONTEND_URL}/dashboard/#session="))
        account = self.frontend.dashboard_of(dashboard_url).account()
        self.assert_equal(
            {key: account[key] for key in account if key != "settings"},
            {
                "name": "signin",
                "avatar": None,
                "twitch": {"login": "signin", "name": "signin", "avatar": None},
                "kick": None,
                "key": account["key"],
                "devices": [],
                "admin": False,
                "premium": False,
                "preview": True,
                "goLive": True,
                "moderating": [],
                "twitchModerators": [],
                "kickModerators": [],
            },
        )


class AccountSignInWithTwitch(TestCase):
    """Sign in with a real Twitch account in a browser and get the account."""

    def run(self):
        self.skip_if_no_twitch()
        if self.context.config.twitch_sign_in() != "real":
            self.skip_if_not_interactive()
        account = self.sign_in().account()
        self.assert_not_equal(account["twitch"]["login"], "")
        self.assert_not_equal(account["name"], "")


class AccountSignInFromAnotherBrowser(AccountTestCase):
    """Fail to sign in when the callback is opened in another browser than the one that started it, or twice."""

    def run(self):
        callback_url, _ = self.frontend.start_sign_in("otherbrowser")
        dashboard_url = self.frontend.finish_sign_in(callback_url, requests.Session())
        self.assert_equal(dashboard_url, f"{FRONTEND_URL}/dashboard/#signInFailed")
        callback_url, browser = self.frontend.start_sign_in("twice")
        self.frontend.finish_sign_in(callback_url, browser)
        replaying = requests.Session()
        replaying.headers["cookie"] = f"moblin_sign_in_state={callback_url.split('state=')[1]}"
        dashboard_url = self.frontend.finish_sign_in(callback_url, replaying)
        self.assert_equal(dashboard_url, f"{FRONTEND_URL}/dashboard/#signInFailed")
        dashboard_url = self.frontend.finish_sign_in(f"{self.backend.url}/auth/twitch/callback", browser)
        self.assert_equal(dashboard_url, f"{FRONTEND_URL}/dashboard/#signInFailed")


class AccountNotSignedIn(TestCase):
    """Refuse every account request without a valid session."""

    def run(self):
        for session in [None, "garbage", "a.b", session_issued_at("ZmFrZS14OjA6MA.x", int(time.time()))]:
            dashboard = Dashboard(self.backend.url, session)
            self.assert_backend_error(401, dashboard.account, "not signed in")
            self.assert_backend_error(401, dashboard.new_moblin_cloud_key, "not signed in")
            self.assert_backend_error(401, dashboard.save_settings, "not signed in")
            self.assert_backend_error(401, dashboard.check_paypal, "not signed in")
            self.assert_backend_error(401, dashboard.disconnect_paypal, "not signed in")
            self.assert_backend_error(401, dashboard.sign_out_everywhere, "not signed in")
            self.assert_backend_error(401, dashboard.delete_account, "not signed in")


class AccountExpiredSession(AccountTestCase):
    """Refuse a session issued more than 30 days ago."""

    def run(self):
        dashboard = self.sign_in("expired")
        dashboard.account()
        old = session_issued_at(dashboard.session, int(time.time()) - 31 * 24 * 60 * 60)
        self.assert_signed_out(Dashboard(self.backend.url, old))


class AccountPageAtSignIn(AccountTestCase):
    """Give the streamer a page with a Moblin Cloud key when signing in, and keep it on later sign ins."""

    def run(self):
        dashboard = self.sign_in("createpage")
        account = dashboard.account()
        self.assert_equal(account["devices"], [])
        self.assert_regex(account["key"], RE_MOBLIN_CLOUD_KEY)
        settings = account["settings"]
        self.assert_equal(
            {key: settings[key] for key in ["connected", "currency", "tips", "spin", "merchantId"]},
            {
                "connected": False,
                "currency": "USD",
                "tips": None,
                "spin": None,
                "merchantId": None,
            },
        )
        self.assert_true(settings["paypal"])
        self.assert_true(settings["paypalSandbox"])
        if self.context.config.twitch_sign_in() == "fake":
            self.assert_equal(self.sign_in("createpage").account(), account)
        page_id = settings["id"]
        self.assert_equal(self.frontend.id_of_twitch_login(account["twitch"]["login"]), page_id)
        self.assert_equal(self.frontend.id_of_twitch_login(account["twitch"]["login"].upper()), page_id)
        profile = self.frontend.viewer(page_id).profile()
        self.assert_equal(profile["twitch"], account["twitch"])
        self.assert_equal(profile["kick"], None)

    def assert_regex(self, text: str, regex: re.Pattern):
        self.assert_true(regex.match(text) is not None, f"{text!r} matches {regex.pattern!r}")


class AccountUnknownPage(TestCase):
    """Answer not found for pages and logins nobody has."""

    def run(self):
        self.assert_backend_error(404, lambda: self.frontend.id_of_twitch_login("nobody_has_this_login"))
        viewer = self.frontend.viewer("AAAAAAAAAAAAAAAA")
        self.assert_backend_error(404, viewer.profile)
        self.assert_equal(viewer.tips(), None)
        self.assert_equal(viewer.spin(), {"spin": None, "subSpins": None, "wheel": None, "connected": False})


class AccountNewMoblinCloudKey(AccountTestCase):
    """Replace the Moblin Cloud key, which stops the old one from working but keeps the page."""

    def run(self):
        dashboard = self.sign_in("newcloudkey")
        account = dashboard.account()
        port = self.backend.port
        old_key = account["key"]
        self.assert_true(streamer_connects(port, old_key))
        new_account = dashboard.new_moblin_cloud_key()
        new_key = new_account["key"]
        self.assert_not_equal(new_key, old_key)
        self.assert_equal(new_account["settings"]["id"], account["settings"]["id"])
        self.assert_equal(dashboard.moblin_cloud_key(), new_key)
        self.assert_false(streamer_connects(port, old_key))
        self.assert_true(streamer_connects(port, new_key))
        self.assert_false(streamer_connects(port, "x"))


class AccountDevices(AccountTestCase):
    """Connect up to 5 devices running Moblin with the one Moblin Cloud key, list them on the dashboard and rename them."""

    def run(self):
        dashboard = self.sign_in("devices")
        key = dashboard.moblin_cloud_key()
        port = self.backend.port
        websockets = [identified(port, key, f"DEVICE-{number}") for number in range(1, 6)]
        try:
            self.assert_true(all(websocket is not None for websocket in websockets))
            self.assert_equal(identified(port, key, "DEVICE-6"), None)
            account = dashboard.account()
            self.assert_equal(
                account["devices"],
                [
                    {
                        "id": f"DEVICE-{number}",
                        "name": f"Moblin {number}",
                        "connected": True,
                        "reconnecting": False,
                        "wheel": False,
                        "lastSeen": None,
                    }
                    for number in range(1, 6)
                ],
            )
            self.assert_true(account["settings"]["connected"])
            self.assert_backend_error(
                404, lambda: dashboard.rename_device("DEVICE-6", "Six"), "no such device"
            )
            account = dashboard.rename_device("DEVICE-2", "Backup")
            self.assert_equal(account["devices"][1]["name"], "Backup")
            self.assert_backend_error(
                400, lambda: dashboard.rename_device("DEVICE-2", " "), "the name must be 1 to 30 characters"
            )
            second = websockets[1]
            assert second is not None
            second.close()
            self.wait_until(
                lambda: not dashboard.account()["devices"][4]["connected"], "the closed device to disconnect"
            )
            device = dashboard.account()["devices"][4]
            self.assert_equal([device["id"], device["name"]], ["DEVICE-2", "Backup"])
            self.assert_true(device["lastSeen"] is not None)
            self.assert_backend_error(
                409,
                lambda: dashboard.remove_device("DEVICE-1"),
                "Turn off Moblin Cloud on the device first.",
            )
            account = dashboard.remove_device("DEVICE-2")
            self.assert_equal(
                [device["id"] for device in account["devices"]],
                [f"DEVICE-{number}" for number in [1, 3, 4, 5]],
            )
        finally:
            for websocket in websockets:
                if websocket is not None:
                    websocket.close()


class AccountSignOutEverywhere(AccountTestCase):
    """Sign out everywhere, which ends all sessions, and sign in again."""

    def run(self):
        first = self.sign_in("signout")
        second = self.sign_in("signout")
        page_id = first.page_id()
        first.sign_out_everywhere()
        self.assert_signed_out(first)
        self.assert_signed_out(second)
        third = self.sign_in("signout")
        self.assert_equal(third.account()["settings"]["id"], page_id)


class AccountDelete(AccountTestCase):
    """Delete the account, which removes its page, ends all sessions and disconnects Moblin."""

    def run(self):
        first = self.sign_in("delete")
        second = self.sign_in("delete")
        page_id = first.page_id()
        key = first.moblin_cloud_key()
        first.save_settings(message="Hello!")
        port = self.backend.port
        self.assert_true(streamer_connects(port, key))
        first.delete_account()
        self.assert_signed_out(first)
        self.assert_signed_out(second)
        self.assert_backend_error(401, first.delete_account, "not signed in")
        self.assert_false(streamer_connects(port, key))
        self.assert_backend_error(404, lambda: self.frontend.id_of_twitch_login("delete"))
        self.assert_backend_error(404, lambda: self.frontend.get(f"/pages/{page_id}"))
        third = self.sign_in("delete")
        account = third.account()
        self.assert_not_equal(account["settings"]["id"], page_id)
        self.assert_equal(account["settings"].get("message"), None)
        self.assert_equal(self.frontend.id_of_twitch_login("delete"), account["settings"]["id"])


class AccountPageLook(AccountTestCase):
    """Change the look of the page in the dashboard and see it on the page."""

    def run(self):
        dashboard = self.sign_in("pagelook")
        page_id = dashboard.page_id()
        settings = dashboard.save_settings(message="  Thanks for watching.  ", accent="purple", font="serif")
        self.assert_equal(settings["message"], "Thanks for watching.")
        self.assert_equal(settings["accent"], "purple")
        self.assert_equal(settings["font"], "serif")
        profile = self.frontend.viewer(page_id).profile()
        self.assert_equal(profile["message"], "Thanks for watching.")
        self.assert_equal(profile["accent"], "purple")
        self.assert_equal(profile["font"], "serif")
        settings = dashboard.save_settings(message="   ", accent="green", font="rounded")
        self.assert_equal(
            [settings["message"], settings["accent"], settings["font"]],
            [None, None, None],
        )
        settings = dashboard.save_settings(message=300 * "å")
        self.assert_equal(settings["message"], 300 * "å")
        invalid = "the message can be at most 300 characters and the accent and font among the supported ones"
        self.assert_backend_error(400, lambda: dashboard.save_settings(message=301 * "x"), invalid)
        self.assert_backend_error(400, lambda: dashboard.save_settings(accent="black"), invalid)
        self.assert_backend_error(400, lambda: dashboard.save_settings(font="comic"), invalid)
        self.assert_equal(dashboard.account()["settings"]["message"], 300 * "å")
        normal = self.sign_in("normallook")
        self.assert_backend_error(
            403, lambda: normal.save_settings(font="serif"), "fonts are only for premium accounts"
        )
        self.assert_equal(normal.save_settings(accent="purple", font="rounded")["font"], None)


class AccountCurrency(AccountTestCase):
    """Change the currency of a page without paid features, which needs no PayPal."""

    def run(self):
        dashboard = self.sign_in("currency")
        self.assert_equal(dashboard.save_settings(currency=" eur ")["currency"], "EUR")
        self.assert_equal(dashboard.save_settings()["currency"], "EUR")
        self.assert_backend_error(
            400, lambda: dashboard.save_settings(currency="XYZ"), "the currency is not supported"
        )
        self.assert_equal(dashboard.save_settings(currency="USD")["currency"], "USD")


class AccountPaidFeaturesNeedPayments(AccountTestCase):
    """Refuse to turn tips or the wheel on before PayPal or Stripe is connected."""

    def run(self):
        dashboard = self.sign_in("paidneedspaypal")
        page_id = dashboard.page_id()
        for changes in [
            {"tips": {"amounts": [100]}},
            {"spin": {"options": [{"spins": 1, "price": 100}]}},
        ]:
            self.assert_backend_error(
                409,
                lambda changes=changes: dashboard.save_settings(**changes),
                "connect PayPal or Stripe first",
            )
        viewer = self.frontend.viewer(page_id)
        self.assert_equal(viewer.tips(), None)
        self.assert_equal(viewer.spin()["spin"], None)


class AccountTestDeliveriesNeedMoblin(AccountTestCase):
    """Refuse the dashboard's free test wheel spin without a connected Moblin, and queue the test tip."""

    def run(self):
        dashboard = self.sign_in("testneedsmoblin")
        empty = {"items": [], "more": False}
        for kind in ["tip", "spin"]:
            self.assert_equal(dashboard.recent(kind), empty)
        self.assert_backend_error(409, dashboard.test_spin, "Connect Moblin first.")
        dashboard.test_tip()
        for kind in ["tip", "spin"]:
            self.assert_equal(dashboard.recent(kind), empty)


class AccountAdmin(AccountTestCase):
    """Show the operator's statistics only to admins."""

    def run(self):
        streamer = self.sign_in("notadmin")
        self.assert_false(streamer.account()["admin"])
        self.assert_backend_error(403, streamer.admin_stats, "not an admin")
        self.assert_backend_error(401, Dashboard(self.backend.url, None).admin_stats, "not signed in")
        admin = self.sign_in("admin")
        self.assert_true(admin.account()["admin"])
        stats = admin.admin_stats()
        self.assert_equal(sorted(stats), ["accounts", "counters", "database", "moblin", "pending", "server"])
        server = stats["server"]
        self.assert_equal(sorted(server), ["integrations", "memory", "now", "requests", "started", "version"])
        self.assert_true(0 < server["started"] <= server["now"])
        served = server["requests"]
        self.assert_equal(sorted(served), ["endpoints", "statuses", "total"])
        self.assert_equal(sum(endpoint["total"] for endpoint in served["endpoints"]), served["total"])
        stats_endpoint = next(e for e in served["endpoints"] if e["path"] == "/admin/stats")
        self.assert_equal(sorted(stats_endpoint), ["path", "statuses", "total"])
        self.assert_true(stats_endpoint["statuses"]["401"] >= 1)
        self.assert_equal(sum(served["statuses"].values()), served["total"])
        self.assert_true(served["statuses"]["401"] >= 1)
        self.assert_true(served["statuses"]["403"] >= 1)
        before = served["total"]
        self.assert_true(admin.admin_stats()["server"]["requests"]["total"] > before)
        self.assert_equal(sorted(server["integrations"]), ["appAttest", "kick", "paypal", "stripe", "twitch"])
        self.assert_equal(sorted(stats["accounts"]), ["paypal", "premium", "stripe", "total"])
        self.assert_true(stats["accounts"]["total"] >= 2)
        self.assert_true(stats["accounts"]["premium"] >= 1)
        self.assert_in("connectedDevices", stats["moblin"])
        self.assert_equal(sorted(stats["pending"]), ["deliveries", "orders"])
        for counter in stats["counters"]:
            self.assert_equal(sorted(counter), ["name", "today", "total", "week"])
            self.assert_true(counter["today"] <= counter["week"] <= counter["total"])


class AccountCors(TestCase):
    """Only let https://cloud.moblin.app call the backend from a browser."""

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
            response = requests.get(f"{self.backend.url}/icons/plain", headers={"origin": origin}, timeout=10)
            self.assert_equal(response.headers.get("access-control-allow-origin"), FRONTEND_URL)


def tests(context: Context):
    return [
        AccountSignIn(context),
        AccountSignInWithTwitch(context),
        AccountSignInFromAnotherBrowser(context),
        AccountNotSignedIn(context),
        AccountExpiredSession(context),
        AccountPageAtSignIn(context),
        AccountUnknownPage(context),
        AccountNewMoblinCloudKey(context),
        AccountDevices(context),
        AccountSignOutEverywhere(context),
        AccountDelete(context),
        AccountPageLook(context),
        AccountCurrency(context),
        AccountPaidFeaturesNeedPayments(context),
        AccountTestDeliveriesNeedMoblin(context),
        AccountAdmin(context),
        AccountCors(context),
    ]
