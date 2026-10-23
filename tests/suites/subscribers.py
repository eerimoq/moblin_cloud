from urllib.parse import parse_qs
from urllib.parse import urlsplit

from ..utils.config import FRONTEND_URL
from ..utils.context import Context
from ..utils.moblin import Moblin
from ..utils.moblin import wheel_option
from ..utils.test_case import TestCase
from ..utils.website import Client
from ..utils.website import Subscriber
from ..utils.website import viewer_of

WHEEL = [wheel_option("Dance"), wheel_option("Sing")]


class SubscribersTestCase(TestCase):
    """A streamer's page with spins for Twitch subscribers, who sign in as made-up users.

    The fake Twitch gives the tier of the login's last digit, 1 to 3, and no subscription
    otherwise. Each test signs in its own viewers, since what they use is counted per month.
    """

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.dashboard = self.sign_in()
        self.page_id = self.dashboard.page_id()
        self.viewer = self.website.viewer(self.page_id)

    def teardown(self):
        if hasattr(self, "dashboard"):
            self.dashboard.update_settings(sub_spins=None)

    @property
    def moblin(self) -> Moblin:
        assert self.context.moblin is not None
        return self.context.moblin

    def subscriber(self, login: str) -> Subscriber:
        return self.website.subscriber(self.page_id, login)

    def connect_moblin(self, wheel: list[dict] | None = None):
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key(), wheel)
        self.wait_until(lambda: self.viewer.spin()["connected"], "Moblin to be connected")
        if wheel is not None:
            self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "the wheel to be on the page")

    def recent_tiers(self, kind: str) -> list[tuple[str, int | None]]:
        return [(item["user"], item.get("tier")) for item in self.dashboard.recent(kind)["items"]]


class SubscribersSignIn(SubscribersTestCase):
    """Sign viewers in with Twitch on the page, with their tier, back to the page also when it fails."""

    def run(self):
        login = self.dashboard.account()["twitch"]["login"]
        self.assert_backend_error(404, lambda: self.website.start_viewer_sign_in("no-such-page"))
        page_url = self.website.fake_viewer_sign_in(self.page_id, "signin2")
        self.assert_true(page_url.startswith(f"{FRONTEND_URL}/twitch/{login}#viewer="), page_url)
        subscriber = self.subscriber("signin2")
        self.assert_equal(
            subscriber.spins(),
            {"platform": "twitch", "name": "signin2", "tier": 2, "allowance": 0, "left": 0},
        )
        self.assert_equal(self.subscriber("signinviewer").spins()["tier"], None)
        response = Client(self.website.url, subscriber.token).request("GET", "/account")
        self.assert_equal(response.status_code, 401)
        other = self.website.fake_sign_in("otherstreamer")
        stranger = Subscriber(self.website.url, other.page_id(), subscriber.token)
        self.assert_backend_error(401, stranger.spins, "Sign in again.")
        callback_url, browser = self.website.start_viewer_sign_in(self.page_id, "signin3")
        state = parse_qs(urlsplit(callback_url).query)["state"][0]
        cancelled = self.website.finish_sign_in(
            f"{self.website.url}/auth/twitch/callback?error=access_denied&state={state}", browser
        )
        self.assert_equal(cancelled, f"{FRONTEND_URL}/twitch/{login}#viewerSignInFailed=twitch")
        self.assert_equal(viewer_of(cancelled), None)


class SubscribersSpinTheWheel(SubscribersTestCase):
    """Let subscribers spin Moblin's wheel without paying, within their tier's spins."""

    def run(self):
        self.dashboard.update_settings(sub_spins={"tier1": 1, "tier2": 0, "tier3": 2})
        self.assert_equal(self.viewer.spin()["subSpins"], {"tier1": 1, "tier2": 0, "tier3": 2})
        self.moblin.disconnect_from_backend()
        self.wait_until(lambda: not self.viewer.spin()["connected"], "Moblin to be disconnected")
        tier1 = self.subscriber("spinfan1")
        self.assert_backend_error(
            404,
            tier1.spin_wheel,
            "The streamer's wheel is not available right now. Try again when they are live.",
        )
        self.connect_moblin()
        self.assert_backend_error(409, tier1.spin_wheel, "The streamer is not taking wheel spins right now.")
        self.assert_equal(tier1.spins()["left"], 1)
        self.connect_moblin(WHEEL)
        start = self.moblin.backend_requests_position()
        self.assert_equal(tier1.spin_wheel(" Land on dance "), {"left": 0})
        request = self.moblin.wait_for_new_backend_request("spinWheelOfLuck", start)
        self.assert_equal(
            [request.body["name"], request.body["count"], request.body.get("message")],
            ["spinfan1", 1, "Land on dance"],
        )
        self.assert_backend_error(409, tier1.spin_wheel, "You have used all your spins this month.")
        self.assert_backend_error(
            403,
            self.subscriber("spinfan2").spin_wheel,
            "Tier 2 subscribers can't spin the wheel here.",
        )
        tier3 = self.subscriber("spinfan3")
        self.assert_equal(tier3.spin_wheel(), {"left": 1})
        self.wait_until(
            lambda: all(item["status"] == "delivered" for item in self.dashboard.recent("spin")["items"][:2]),
            "the spins to be in Moblin",
        )
        self.assert_equal(self.recent_tiers("spin")[:2], [("spinfan3", 3), ("spinfan1", 1)])


class SubscribersTurnedOff(SubscribersTestCase):
    """Refuse subscribers' spins once the streamer turns them off."""

    def run(self):
        self.dashboard.update_settings(sub_spins={"tier1": 1, "tier2": 1, "tier3": 1})
        subscriber = self.subscriber("offfan1")
        self.assert_equal(subscriber.spins()["allowance"], 1)
        self.dashboard.update_settings(message="Kept", sub_spins=None)
        settings = self.dashboard.account()["settings"]
        self.assert_equal([settings["subSpins"], settings["message"]], [None, "Kept"])
        self.assert_equal(self.viewer.spin()["subSpins"], None)
        self.assert_equal(subscriber.spins()["allowance"], 0)
        self.assert_backend_error(
            400,
            subscriber.spin_wheel,
            "The streamer no longer lets subscribers spin the wheel. Reload the page.",
        )
        self.assert_backend_error(
            400,
            lambda: self.dashboard.update_settings(sub_spins={"tier1": 0, "tier2": 0, "tier3": 0}),
        )
        self.assert_backend_error(
            400,
            lambda: self.dashboard.update_settings(sub_spins={"tier1": 101, "tier2": 0, "tier3": 0}),
        )
        self.dashboard.update_settings(message=None)


class SubscribersKeptOnRestart(SubscribersTestCase):
    """Keep the settings, viewers' sign ins and what they used when the backend restarts."""

    def run(self):
        self.dashboard.update_settings(sub_spins={"tier1": 2, "tier2": 2, "tier3": 2})
        self.connect_moblin(WHEEL)
        subscriber = self.subscriber("restartfan1")
        start = self.moblin.backend_requests_position()
        self.assert_equal(subscriber.spin_wheel(), {"left": 1})
        self.moblin.wait_for_new_backend_request("spinWheelOfLuck", start)
        self.backend.restart()
        self.assert_equal(
            self.dashboard.account()["settings"]["subSpins"],
            {"enabled": True, "tier1": 2, "tier2": 2, "tier3": 2},
        )
        self.assert_equal(
            subscriber.spins(),
            {"platform": "twitch", "name": "restartfan1", "tier": 1, "allowance": 2, "left": 1},
        )
        self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "Moblin to reconnect with its wheel")
        self.assert_equal(subscriber.spin_wheel(), {"left": 0})


class SubscribersCounted(SubscribersTestCase):
    """Count subscribers' spins, and why they were refused, for the admin page."""

    def run(self):
        admin = self.sign_in("admin")

        def totals() -> dict[str, int]:
            return {counter["name"]: counter["total"] for counter in admin.admin_stats()["counters"]}

        self.dashboard.update_settings(sub_spins={"tier1": 1, "tier2": 0, "tier3": 1})
        self.connect_moblin(WHEEL)
        before = totals()
        subscriber = self.subscriber("countfan1")
        subscriber.spin_wheel()
        self.assert_backend_error(409, subscriber.spin_wheel)
        self.assert_backend_error(403, self.subscriber("countfan2").spin_wheel)
        self.assert_backend_error(403, self.subscriber("countfanviewer").spin_wheel)
        after = totals()
        for name in [
            "subSpins",
            "subSpinsFailed:no-spins-left",
            "subSpinsFailed:tier-excluded",
            "subSpinsFailed:not-subscribed",
        ]:
            self.assert_equal(after.get(name, 0) - before.get(name, 0), 1, name)


def tests(context: Context):
    return [
        SubscribersSignIn(context),
        SubscribersSpinTheWheel(context),
        SubscribersTurnedOff(context),
        SubscribersKeptOnRestart(context),
        SubscribersCounted(context),
    ]
