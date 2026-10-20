from urllib.parse import parse_qs
from urllib.parse import urlsplit

from ..utils.config import FRONTEND_URL
from ..utils.config import Capability
from ..utils.context import Context
from ..utils.moblin import Moblin
from ..utils.moblin import wheel_option
from ..utils.test_case import TestCase
from ..utils.website import Client
from ..utils.website import Subscriber
from ..utils.website import viewer_of

WHEEL = [wheel_option("Dance"), wheel_option("Sing")]
SONG = "https://music.apple.com/us/album/take-on-me/380907762?i=380907765"
SONG_LINK = "https://music.apple.com/song/380907765?i=380907765"
MISSING_SONG = "https://music.apple.com/us/song/does-not-exist/99999999999"


class SubscribersTestCase(TestCase):
    """A streamer's page with songs and spins for Twitch subscribers, who sign in as made-up users.

    The fake Twitch sign in gives the tier of the login's last digit, 1 to 3, and no subscription
    otherwise. Each test signs in its own viewers, since what they use is counted per month.
    """

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.dashboard = self.sign_in()
        self.page_id = self.dashboard.page_id()
        self.viewer = self.website.viewer(self.page_id)

    def teardown(self):
        if hasattr(self, "dashboard"):
            self.dashboard.update_settings(sub_songs=None, sub_spins=None)

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
        login = self.dashboard.account()["login"]
        self.assert_backend_error(404, lambda: self.website.start_viewer_sign_in("no-such-page"))
        page_url = self.website.fake_viewer_sign_in(self.page_id, "signin2")
        self.assert_true(page_url.startswith(f"{FRONTEND_URL}/twitch/{login}#viewer="), page_url)
        subscriber = self.subscriber("signin2")
        self.assert_equal(subscriber.songs(), {"name": "signin2", "tier": 2, "allowance": 0, "left": 0})
        self.assert_equal(self.subscriber("signinviewer").songs()["tier"], None)
        response = Client(self.website.url, subscriber.token).request("GET", "/account")
        self.assert_equal(response.status_code, 401)
        other = self.website.fake_sign_in("otherstreamer")
        stranger = Subscriber(self.website.url, other.page_id(), subscriber.token)
        self.assert_backend_error(401, stranger.songs, "Sign in with Twitch again.")
        callback_url, browser = self.website.start_viewer_sign_in(self.page_id, "signin3")
        state = parse_qs(urlsplit(callback_url).query)["state"][0]
        cancelled = self.website.finish_sign_in(
            f"{self.website.url}/auth/twitch/callback?error=access_denied&state={state}", browser
        )
        self.assert_equal(cancelled, f"{FRONTEND_URL}/twitch/{login}#viewerSignInFailed")
        self.assert_equal(viewer_of(cancelled), None)


class SubscribersRequestSongs(SubscribersTestCase):
    """Let subscribers add songs to Moblin's queue without paying, within their tier's minutes."""

    def run(self):
        self.skip_if_missing_capability(Capability.APPLE_MUSIC)
        self.dashboard.update_settings(sub_songs={"tier1": 1, "tier2": 0, "tier3": 3})
        self.assert_equal(
            self.viewer.get(f"/pages/{self.page_id}/songs")["subSongs"],
            {"tier1": 1, "tier2": 0, "tier3": 3},
        )
        self.connect_moblin()
        self.moblin.clear_music()
        tier1 = self.subscriber("songfan1")
        self.assert_equal(tier1.songs(), {"name": "songfan1", "tier": 1, "allowance": 1, "left": 1})
        self.assert_backend_error(
            400, lambda: tier1.request_song("Take on me"), "Paste the Apple Music share link of a song."
        )
        self.assert_backend_error(
            422,
            lambda: tier1.request_song(MISSING_SONG),
            "That song is not available on the streamer's Apple Music.",
        )
        self.assert_equal(tier1.songs()["left"], 1)
        start = self.moblin.backend_requests_position()
        self.assert_equal(tier1.request_song(SONG, " For the drive "), {"left": 0})
        add = self.moblin.wait_for_new_backend_request("add", start)
        self.assert_equal(
            [add.body["title"], add.body["user"], add.body.get("message")],
            [SONG_LINK, "songfan1", "For the drive"],
        )
        self.wait_until(
            lambda: [song.get("user") for song in self.viewer.queue()["songs"]] == ["songfan1"],
            "the song to be in the queue on the page",
        )
        self.assert_backend_error(
            409, lambda: tier1.request_song(SONG), "You have used all your minutes this month."
        )
        self.assert_backend_error(
            403,
            lambda: self.subscriber("songfan2").request_song(SONG),
            "Tier 2 subscribers can't request songs here.",
        )
        self.assert_backend_error(
            403,
            lambda: self.subscriber("songfanviewer").request_song(SONG),
            "Only Twitch subscribers of this streamer can request songs.",
        )
        tier3 = self.subscriber("songfan3")
        self.assert_equal(tier3.request_song(SONG), {"left": 2})
        self.assert_equal(tier3.request_song(SONG), {"left": 1})
        self.assert_equal(tier3.songs()["left"], 1)
        self.assert_equal(tier1.spins()["left"], 0)
        self.wait_until(
            lambda: all(item["status"] == "delivered" for item in self.dashboard.recent("song")["items"][:3]),
            "the songs to be in Moblin",
        )
        self.assert_equal(self.recent_tiers("song")[:3], [("songfan3", 3), ("songfan3", 3), ("songfan1", 1)])

    def teardown(self):
        super().teardown()
        if self.context.moblin is not None and self.moblin.has_capability(Capability.APPLE_MUSIC):
            self.moblin.clear_music()


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
        self.assert_equal(tier3.songs()["allowance"], 0)
        self.wait_until(
            lambda: all(item["status"] == "delivered" for item in self.dashboard.recent("spin")["items"][:2]),
            "the spins to be in Moblin",
        )
        self.assert_equal(self.recent_tiers("spin")[:2], [("spinfan3", 3), ("spinfan1", 1)])


class SubscribersTurnedOff(SubscribersTestCase):
    """Refuse subscribers' songs and spins once the streamer turns them off."""

    def run(self):
        self.dashboard.update_settings(
            sub_songs={"tier1": 1, "tier2": 1, "tier3": 1}, sub_spins={"tier1": 1, "tier2": 1, "tier3": 1}
        )
        subscriber = self.subscriber("offfan1")
        self.assert_equal(subscriber.songs()["allowance"], 1)
        self.dashboard.update_settings(title="Kept", sub_songs=None, sub_spins=None)
        settings = self.dashboard.account()["settings"]
        self.assert_equal(
            [settings["subSongs"], settings["subSpins"], settings["title"]], [None, None, "Kept"]
        )
        self.assert_equal(self.viewer.get(f"/pages/{self.page_id}/songs")["subSongs"], None)
        self.assert_equal(self.viewer.spin()["subSpins"], None)
        self.assert_equal(subscriber.songs()["allowance"], 0)
        self.assert_backend_error(
            400,
            lambda: subscriber.request_song(SONG),
            "The streamer no longer takes song requests from subscribers. Reload the page.",
        )
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
            lambda: self.dashboard.update_settings(sub_songs={"tier1": 101, "tier2": 0, "tier3": 0}),
        )
        self.dashboard.update_settings(title=None)


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
            self.dashboard.account()["settings"]["subSpins"], {"tier1": 2, "tier2": 2, "tier3": 2}
        )
        self.assert_equal(subscriber.spins(), {"name": "restartfan1", "tier": 1, "allowance": 2, "left": 1})
        self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "Moblin to reconnect with its wheel")
        self.assert_equal(subscriber.spin_wheel(), {"left": 0})


class SubscribersSongRequestsOff(SubscribersTestCase):
    """Refuse subscribers' songs while song requests are off in Moblin."""

    def run(self):
        self.skip_if_missing_capability(Capability.APPLE_MUSIC)
        if not self.moblin.is_fake():
            self.skip_if_not_interactive()
        self.dashboard.update_settings(sub_songs={"tier1": 2, "tier2": 2, "tier3": 2})
        self.connect_moblin()
        self.moblin.change_song_requests(False)
        self.wait_until(lambda: not self.viewer.queue()["songRequests"], "song requests to be off")
        subscriber = self.subscriber("offmoblin1")
        self.assert_backend_error(
            409,
            lambda: subscriber.request_song(SONG),
            "The streamer is not taking song requests right now.",
        )
        self.assert_equal(subscriber.songs()["left"], 2)
        self.moblin.change_song_requests(True)
        self.wait_until(lambda: self.viewer.queue()["songRequests"], "song requests to be on again")
        self.assert_equal(subscriber.request_song(SONG), {"left": 1})

    def teardown(self):
        super().teardown()
        if self.context.moblin is not None and self.moblin.is_fake():
            self.moblin.change_song_requests(True)
            self.moblin.clear_music()


class SubscribersAlongsidePaidSongs(SubscribersTestCase):
    """Offer paid song requests and subscribers' songs on the same page, without one getting in the other's way."""

    def run(self):
        self.skip_if_no_paypal()
        self.skip_if_missing_capability(Capability.APPLE_MUSIC)
        settings = self.context.page_with_paypal(self.dashboard)
        self.dashboard.update_settings(
            currency="USD", paywall={"price": 150}, sub_songs={"tier1": 1, "tier2": 1, "tier3": 1}
        )
        answer = self.viewer.get(f"/pages/{self.page_id}/songs")
        self.assert_equal(answer["paywall"]["merchantId"], settings["merchantId"])
        self.assert_equal(answer["subSongs"], {"tier1": 1, "tier2": 1, "tier3": 1})
        self.connect_moblin()
        self.moblin.clear_music()
        self.assert_true(self.viewer.order_song(SONG, "Payer"))
        subscriber = self.subscriber("paidtoo1")
        start = self.moblin.backend_requests_position()
        self.assert_equal(subscriber.request_song(SONG), {"left": 0})
        add = self.moblin.wait_for_new_backend_request("add", start)
        self.assert_equal(add.body["user"], "paidtoo1")
        self.assert_true(self.viewer.order_song(SONG, "Payer again"))

    def teardown(self):
        if hasattr(self, "dashboard"):
            self.dashboard.update_settings(paywall=None)
        super().teardown()
        if self.context.moblin is not None and self.moblin.has_capability(Capability.APPLE_MUSIC):
            self.moblin.clear_music()


class SubscribersCounted(SubscribersTestCase):
    """Count subscribers' songs and spins, and why they were refused, for the admin page."""

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
        SubscribersRequestSongs(context),
        SubscribersSpinTheWheel(context),
        SubscribersTurnedOff(context),
        SubscribersKeptOnRestart(context),
        SubscribersSongRequestsOff(context),
        SubscribersAlongsidePaidSongs(context),
        SubscribersCounted(context),
    ]
