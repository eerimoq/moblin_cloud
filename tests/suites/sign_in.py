import hashlib
import re
import secrets
from urllib.parse import parse_qs
from urllib.parse import urlsplit

import requests

from ..utils.config import FRONTEND_URL
from ..utils.context import Context
from ..utils.fake_twitch import user_id_of
from ..utils.frontend import Dashboard
from ..utils.frontend import viewer_of
from ..utils.test_case import TestCase
from .account import unpadded_base64url

PORT = 49152
FAILED = "Signing in failed. Try again."


def challenge_of(verifier: str) -> str:
    return unpadded_base64url(hashlib.sha256(verifier.encode()).digest())


class SignInTestCase(TestCase):
    """mobtop signing in, which has no browser of its own to end up in the dashboard with."""

    def code(self, login: str, verifier: str) -> str:
        """The code the browser hands to mobtop's port after signing in."""
        returned = urlsplit(self.frontend.terminal_sign_in(login, PORT, challenge_of(verifier)))
        self.assert_equal(
            [returned.scheme, returned.netloc, returned.path], ["http", f"127.0.0.1:{PORT}", "/"]
        )
        return parse_qs(returned.query)["code"][0]

    def login_status(self, **params: str) -> int:
        response = requests.get(
            f"{self.backend.url}/auth/twitch/login", params=params, allow_redirects=False, timeout=30
        )
        return response.status_code


class SignInFromTheTerminal(SignInTestCase):
    """Sign mobtop in through a browser on the same machine, where the code the browser hands over only gives the session to who has the verifier, once."""

    def run(self):
        verifier = secrets.token_urlsafe(32)
        code = self.code("terminal", verifier)
        mobtop = self.frontend.session_of_code(code, verifier)
        self.assert_equal(mobtop.account()["twitch"]["login"], "terminal")
        self.assert_equal(mobtop.page_id(), self.sign_in("terminal").page_id())
        self.assert_backend_error(400, lambda: self.frontend.session_of_code(code, verifier), FAILED)
        stolen = self.code("terminal", verifier)
        self.assert_backend_error(
            400, lambda: self.frontend.session_of_code(stolen, secrets.token_urlsafe(32)), FAILED
        )
        self.assert_backend_error(400, lambda: self.frontend.session_of_code(stolen, verifier), FAILED)
        self.assert_backend_error(400, lambda: self.frontend.session_of_code("unknown", verifier), FAILED)


class SignInFromTheTerminalRefused(SignInTestCase):
    """Refuse a terminal sign in without both a port and a well-formed challenge, or mixed with a viewer's or a link's, and tell mobtop when the streamer cancels."""

    def run(self):
        challenge = challenge_of(secrets.token_urlsafe(32))
        page_id = self.sign_in("terminalrefused").page_id()
        self.assert_equal(self.login_status(port=str(PORT), challenge=challenge), 303)
        for params in [
            {"port": str(PORT)},
            {"challenge": challenge},
            {"port": str(PORT), "challenge": challenge[:-1]},
            {"port": str(PORT), "challenge": challenge[:-1] + "!"},
            {"port": "0x1", "challenge": challenge},
            {"port": str(PORT), "challenge": challenge, "link": "1"},
            {"port": str(PORT), "challenge": challenge, "page": page_id},
        ]:
            self.assert_equal(self.login_status(**params), 400, f"{params}")
        callback_url, browser = self.frontend._start_sign_in(  # pylint: disable=protected-access
            {"port": str(PORT), "challenge": challenge}, "terminalrefused"
        )
        state = parse_qs(urlsplit(callback_url).query)["state"][0]
        returned = self.frontend.finish_sign_in(
            f"{self.backend.url}/auth/twitch/callback?error=access_denied&state={state}", browser
        )
        self.assert_equal(returned, f"http://127.0.0.1:{PORT}/?signInFailed")


class SignInWithACode(SignInTestCase):
    """Sign mobtop in on another device, once the streamer types the code it shows into the Connect page and approves it."""

    def run(self):
        connect = self.frontend.start_connect()
        self.assert_equal(sorted(connect), ["deviceCode", "expiresIn", "interval", "url", "userCode"])
        self.assert_equal(
            [connect["url"], connect["interval"], connect["expiresIn"]], [f"{FRONTEND_URL}/connect/", 5, 600]
        )
        self.assert_true(
            re.fullmatch(r"[BCDFGHJKLMNPQRSTVWXZ]{4}-[BCDFGHJKLMNPQRSTVWXZ]{4}", connect["userCode"]),
            connect["userCode"],
        )
        self.assert_not_in(connect["userCode"].replace("-", ""), connect["deviceCode"])
        self.assert_is_none(self.frontend.connect_session(connect["deviceCode"]))
        streamer = self.sign_in("connectcode")
        typed = connect["userCode"].lower().replace("-", " ")
        self.assert_equal(streamer.decide_connect(typed, True), {})
        mobtop = self.frontend.connect_session(connect["deviceCode"])
        assert mobtop is not None
        self.assert_equal(mobtop.page_id(), streamer.page_id())
        gone = "The code expired. Start signing in again."
        self.assert_backend_error(410, lambda: self.frontend.connect_session(connect["deviceCode"]), gone)
        self.assert_backend_error(410, lambda: self.frontend.connect_session("unknown"), gone)
        self.assert_backend_error(404, lambda: streamer.decide_connect(connect["userCode"], True))
        streamer.sign_out_everywhere()
        self.assert_backend_error(401, mobtop.account, "not signed in")


class SignInWithACodeDenied(SignInTestCase):
    """Sign nobody in with a code the streamer denies, and limit how many codes a streamer may try."""

    def run(self):
        connect = self.frontend.start_connect()
        streamer = self.sign_in("connectdenied")
        no_code = "There is no such code. Check it, or start signing in again in mobtop."
        self.assert_backend_error(
            401, lambda: Dashboard(self.backend.url, None).decide_connect(connect["userCode"], True)
        )
        self.assert_backend_error(404, lambda: streamer.decide_connect("BBBB-BBBB", True), no_code)
        self.assert_is_none(self.frontend.connect_session(connect["deviceCode"]))
        self.assert_equal(streamer.decide_connect(connect["userCode"], False), {})
        self.assert_backend_error(404, lambda: streamer.decide_connect(connect["userCode"], True), no_code)
        self.assert_backend_error(
            403, lambda: self.frontend.connect_session(connect["deviceCode"]), "Signing in was denied."
        )
        self.assert_backend_error(410, lambda: self.frontend.connect_session(connect["deviceCode"]))
        for _ in range(7):
            self.assert_backend_error(404, lambda: streamer.decide_connect("BBBB-BBBB", True), no_code)
        self.assert_backend_error(
            429, lambda: streamer.decide_connect("BBBB-BBBB", True), "Too many tries. Try again in a minute."
        )
        other = self.sign_in("connectdeniedother")
        self.assert_backend_error(404, lambda: other.decide_connect("BBBB-BBBB", True), no_code)


class SignInWhenTwitchIsDown(SignInTestCase):
    """Return to the dashboard with signInFailed when Twitch does not answer, and tell a streamer adding an assistant to try again."""

    def run(self):
        streamer = self.sign_in("twitchdown")
        fake = self.backend.fake_twitch
        fake.down = True
        callback_url, browser = self.frontend.start_sign_in("twitchdownlater")
        returned = self.frontend.finish_sign_in(callback_url, browser)
        self.assert_equal(returned, f"{FRONTEND_URL}/dashboard/#signInFailed")
        self.assert_backend_error(
            502,
            lambda: streamer.add_assistant("twitch", "twitchdownmod"),
            "Couldn't look the Twitch user up. Try again.",
        )
        self.assert_equal(streamer.account()["twitch"]["login"], "twitchdown")
        fake.down = False
        self.assert_equal(self.sign_in("twitchdownlater").account()["twitch"]["login"], "twitchdownlater")
        streamer.add_assistant("twitch", "twitchdownmod")

    def teardown(self):
        self.backend.fake_twitch.down = False


class SignInOnlyAllowed(SignInTestCase):
    """Once an admin limits sign in, let in only the allowed accounts and the admins, list everyone else as waiting until allowed, sign out who is removed and keep viewers signing in on pages."""

    def refused(self, login: str) -> str:
        callback_url, browser = self.frontend.start_sign_in(login)
        return self.frontend.finish_sign_in(callback_url, browser)

    def waiting(self, admin: Dashboard) -> list[str]:
        return [person["login"] for person in admin.admin_stats()["signIn"]["requests"]]

    def run(self):
        refused = f"{FRONTEND_URL}/dashboard/#signInRefused"
        admin = self.sign_in("admin")
        before = self.sign_in("allowedbefore")
        premium = self.sign_in("pagelook")
        self.assert_backend_error(403, lambda: before.restrict_sign_in(True), "not an admin")
        state = admin.restrict_sign_in(True)
        self.assert_true(state["restricted"])
        allowed = [(person["platform"], person["userId"]) for person in state["allowed"]]
        self.assert_in(("twitch", user_id_of("allowedbefore")), allowed)
        self.assert_equal(self.sign_in("allowedbefore").page_id(), before.page_id())
        admin.remove_sign_in("twitch", user_id_of("pagelook"))
        self.assert_backend_error(401, premium.account, "not signed in")
        self.assert_equal(self.refused("pagelook"), refused)
        self.assert_equal(self.refused("allowednew"), refused)
        returned = self.frontend.terminal_sign_in("allowednew", PORT, challenge_of(secrets.token_urlsafe(32)))
        self.assert_equal(returned, f"http://127.0.0.1:{PORT}/?signInRefused")
        self.assert_equal(sorted(self.waiting(admin)), ["allowednew", "pagelook"])
        self.assert_true(
            viewer_of(self.frontend.fake_viewer_sign_in(before.page_id(), "allowedviewer")) is not None
        )
        self.assert_backend_error(404, lambda: admin.allow_sign_in("twitch", user_id_of("unknown")))
        admin.allow_sign_in("twitch", user_id_of("allowednew"))
        self.assert_equal(self.sign_in("allowednew").account()["twitch"]["login"], "allowednew")
        admin.dismiss_sign_in("twitch", user_id_of("pagelook"))
        self.assert_equal(self.waiting(admin), [])
        admin.remove_sign_in("twitch", user_id_of("admin"))
        self.assert_true(self.sign_in("admin").account()["admin"])
        admin.restrict_sign_in(False)
        self.assert_equal(self.sign_in("pagelook").page_id(), premium.page_id())

    def teardown(self):
        self.sign_in("admin").restrict_sign_in(False)


def tests(context: Context):
    return [
        SignInFromTheTerminal(context),
        SignInFromTheTerminalRefused(context),
        SignInWithACode(context),
        SignInWithACodeDenied(context),
        SignInWhenTwitchIsDown(context),
        SignInOnlyAllowed(context),
    ]
