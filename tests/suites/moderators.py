from ..utils.config import Capability
from ..utils.context import Context
from ..utils.moblin import Moblin
from ..utils.test_case import TestCase
from ..utils.website import Dashboard

SONG = "https://music.apple.com/us/album/take-on-me/380907762?i=380907765"
SONG_LINK = "https://music.apple.com/song/380907765?i=380907765"
MISSING_SONG = "https://music.apple.com/us/song/does-not-exist/99999999999"
NOT_MODERATOR = "Only the streamer and the moderators they turned on can do this."


class ModeratorsTestCase(TestCase):
    """A streamer and a made-up Twitch user they add as a moderator by username.

    The fake Twitch resolves any login to a user, standing in for Twitch's user lookup. Each test has
    its own streamer, since song lookups are limited per page and minute.
    """

    login = "streamer"

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.dashboard = self.sign_in(self.login)
        self.page_id = self.dashboard.page_id()
        self.viewer = self.website.viewer(self.page_id)
        self.moderator_login = f"mod_{self.login}"
        self.moderator = self.sign_in(self.moderator_login)
        self.moderator_id = f"fake-{self.moderator_login}"

    def teardown(self):
        if hasattr(self, "moderator_id") and self.listed_moderators():
            self.dashboard.remove_moderator("twitch", self.moderator_id)

    def add_moderator(self):
        account = self.dashboard.add_moderator("twitch", self.moderator_login)
        self.assert_equal([moderator["id"] for moderator in account["twitchModerators"]], [self.moderator_id])

    @property
    def moblin(self) -> Moblin:
        assert self.context.moblin is not None
        return self.context.moblin

    def listed_moderators(self) -> list[dict]:
        return [
            moderator
            for moderator in self.dashboard.account()["twitchModerators"]
            if moderator["login"] == self.moderator_login
        ]

    def moderating(self) -> list[dict]:
        return [
            channel for channel in self.moderator.account()["moderating"] if channel["id"] == self.page_id
        ]

    def connect_moblin(self):
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key())
        self.wait_until(lambda: self.viewer.spin()["connected"], "Moblin to be connected")

    def wait_for_queue(self, users: list[str]):
        self.wait_until(
            lambda: [song.get("user") for song in self.viewer.queue()["songs"]] == users,
            f"the queue on the page to be {users}",
        )


class ModeratorsAdded(ModeratorsTestCase):
    """Let a moderator help only once the streamer added them by username, and not once removed."""

    login = "modon"

    def run(self):
        self.assert_equal(self.listed_moderators(), [])
        self.assert_equal(self.moderating(), [])
        self.assert_backend_error(
            403, lambda: self.moderator.moderate_music(self.page_id, "play"), NOT_MODERATOR
        )
        self.assert_backend_error(
            403, lambda: self.moderator.add_free_song(self.page_id, SONG), NOT_MODERATOR
        )
        self.assert_backend_error(
            403, lambda: self.moderator.moderated_recent(self.page_id, "song"), NOT_MODERATOR
        )
        self.assert_backend_error(
            404,
            lambda: self.dashboard.add_moderator("twitch", "nobody"),
            "There is no Twitch user named nobody.",
        )
        self.assert_backend_error(
            400, lambda: self.dashboard.add_moderator("twitch", "bad name"), "Enter a Twitch username."
        )
        self.assert_backend_error(
            409,
            lambda: self.dashboard.add_moderator("twitch", self.login),
            "That is your own Twitch account.",
        )
        self.assert_backend_error(
            404, lambda: self.dashboard.remove_moderator("twitch", "fake-nobody"), "no such moderator"
        )
        self.assert_backend_error(
            409, lambda: self.dashboard.add_moderator("kick", "someone"), "Link Kick first."
        )
        self.add_moderator()
        self.assert_equal(
            self.listed_moderators(),
            [{"id": self.moderator_id, "login": self.moderator_login, "name": self.moderator_login}],
        )
        streamer = self.dashboard.account()
        self.assert_equal(
            self.moderating(),
            [
                {
                    "id": self.page_id,
                    "platform": "twitch",
                    "name": streamer["twitch"]["name"],
                    "login": streamer["twitch"]["login"],
                    "avatar": streamer["twitch"]["avatar"],
                    "twitch": streamer["twitch"],
                    "kick": streamer["kick"],
                }
            ],
        )
        self.assert_equal(self.moderator.moderated_recent(self.page_id, "song")["more"], False)
        self.assert_backend_error(
            401, lambda: Dashboard(self.website.url, "garbage").moderate_music(self.page_id, "play")
        )
        other = self.sign_in("otherstreamer")
        self.assert_backend_error(403, lambda: other.moderate_music(self.page_id, "play"), NOT_MODERATOR)
        self.assert_backend_error(403, lambda: other.moderated_recent(self.page_id, "song"), NOT_MODERATOR)
        account = self.dashboard.remove_moderator("twitch", self.moderator_id)
        self.assert_equal(account["twitchModerators"], [])
        self.assert_equal(self.listed_moderators(), [])
        self.assert_equal(self.moderating(), [])
        self.assert_backend_error(
            403, lambda: self.moderator.moderate_music(self.page_id, "play"), NOT_MODERATOR
        )


class ModeratorsManageTheQueue(ModeratorsTestCase):
    """Let a moderator and the streamer pause, play, pick, remove and clear songs in Moblin's queue."""

    login = "modqueue"

    def run(self):
        self.skip_if_missing_capability(Capability.APPLE_MUSIC)
        self.add_moderator()
        self.moblin.disconnect_from_backend()
        self.wait_until(lambda: not self.viewer.spin()["connected"], "Moblin to be disconnected")
        self.assert_backend_error(
            404,
            lambda: self.moderator.moderate_music(self.page_id, "play"),
            "The streamer is offline right now. Try again when they are live.",
        )
        self.connect_moblin()
        self.moblin.clear_music()
        self.moderator.add_free_song(self.page_id, SONG)
        self.dashboard.add_free_song(self.page_id, SONG)
        self.wait_for_queue([self.moderator_login, self.login])
        self.assert_backend_error(
            400, lambda: self.moderator.moderate_music(self.page_id, "remove"), "Pick a song in the queue."
        )
        self.assert_backend_error(
            400, lambda: self.moderator.moderate_music(self.page_id, "shuffle"), "Unknown command."
        )
        start = self.moblin.backend_requests_position()
        queue = self.moderator.moderate_music(self.page_id, "pause")
        self.moblin.wait_for_new_backend_request("pause", start)
        self.assert_equal([song["user"] for song in queue["songs"]], [self.moderator_login, self.login])
        self.assert_equal(queue["songRequests"], True)
        if self.moblin.is_fake():
            self.assert_equal(queue["playing"], False)
        queue = self.moderator.moderate_music(self.page_id, "play")
        self.moblin.wait_for_new_backend_request("play", start)
        if self.moblin.is_fake():
            self.assert_equal(queue["playing"], True)
        first, second = (song["id"] for song in queue["songs"])
        queue = self.moderator.moderate_music(self.page_id, "select", second)
        select = self.moblin.wait_for_new_backend_request("select", start)
        self.assert_equal(select.body, {"id": second})
        if self.moblin.is_fake():
            self.assert_equal(queue["currentSongIndex"], 1)
        queue = self.moderator.moderate_music(self.page_id, "remove", first)
        remove = self.moblin.wait_for_new_backend_request("remove", start)
        self.assert_equal(remove.body, {"id": first})
        self.assert_equal([song["id"] for song in queue["songs"]], [second])
        queue = self.dashboard.moderate_music(self.page_id, "clear")
        self.moblin.wait_for_new_backend_request("clear", start)
        self.assert_equal(queue["songs"], [])
        self.wait_for_queue([])

    def teardown(self):
        super().teardown()
        if self.context.moblin is not None and self.moblin.has_capability(Capability.APPLE_MUSIC):
            self.moblin.clear_music()


class ModeratorsUseTheStreamersDashboard(ModeratorsTestCase):
    """Show a moderator the streamer's dashboard without payments, and let them change the rest.

    The streamer has no PayPal or Stripe, so the paid features can't be turned on by anyone.
    """

    login = "moddash"

    def run(self):
        self.dashboard.update_settings(message="Mine", sub_spins=None)
        for call in [
            lambda: self.moderator.moderated_account(self.page_id),
            lambda: self.moderator.moderated_settings(self.page_id, message="Theirs"),
            lambda: self.moderator.moderated_test_tip(self.page_id),
            lambda: self.moderator.moderated_test_spin(self.page_id),
            lambda: self.moderator.moderated_remove_image(self.page_id),
        ]:
            self.assert_backend_error(403, call, NOT_MODERATOR)
        self.add_moderator()
        own = self.dashboard.account()
        account = self.moderator.moderated_account(self.page_id)
        self.assert_equal(
            [
                account["name"],
                account["twitch"],
                account["key"],
                account["devices"],
                account["twitchModerators"],
                account["moderating"],
            ],
            [own["name"], own["twitch"], None, [], [], []],
        )
        self.assert_equal([account["admin"], account["premium"]], [False, own["premium"]])
        settings = account["settings"]
        self.assert_equal(settings["id"], self.page_id)
        self.assert_equal(settings["message"], "Mine")
        self.assert_equal(settings["currency"], own["settings"]["currency"])
        self.assert_equal(
            [
                settings["paypal"],
                settings["merchantId"],
                settings["paypalPending"],
                settings["stripe"],
                settings["stripeAccountId"],
                settings["stripePending"],
            ],
            [False, None, False, False, None, False],
        )
        self.assert_equal(settings["payable"], False)
        for paid in [
            {"songs": {"price": 100}},
            {"tips": {"amounts": [100]}},
            {"spin": {"options": [{"spins": 1, "price": 100}]}},
        ]:
            self.assert_backend_error(
                409,
                lambda paid=paid: self.moderator.moderated_settings(self.page_id, message="Theirs", **paid),
                "connect PayPal or Stripe first",
            )
        settings = self.moderator.moderated_settings(
            self.page_id,
            message="Theirs",
            currency="EUR" if own["settings"]["currency"] != "EUR" else "USD",
            sub_spins={"tier1": 1, "tier2": 2, "tier3": 3},
        )
        self.assert_equal(settings["message"], "Theirs")
        self.assert_equal(settings["subSpins"], {"enabled": True, "tier1": 1, "tier2": 2, "tier3": 3})
        self.assert_equal(settings["merchantId"], None)
        self.assert_equal(
            settings["currency"], own["settings"]["currency"], "currency unchanged by a moderator"
        )
        own = self.dashboard.account()
        self.assert_equal(own["settings"]["message"], "Theirs")
        self.assert_equal(own["settings"]["message"], "Theirs")
        self.assert_equal(own["settings"]["subSpins"], {"enabled": True, "tier1": 1, "tier2": 2, "tier3": 3})
        self.assert_backend_error(
            409, lambda: self.moderator.moderated_test_spin(self.page_id), "Connect Moblin first."
        )
        self.moderator.moderated_remove_image(self.page_id)
        self.connect_moblin()
        start = self.moblin.backend_requests_position()
        self.moderator.moderated_test_tip(self.page_id, " Checking ")
        tip = self.moblin.wait_for_new_backend_request("tip", start)
        self.assert_equal(
            [tip.body["name"], tip.body["message"], tip.body["amount"]],
            [self.moderator_login, "Checking", "5.00"],
        )
        self.assert_equal(self.moderator.moderated_recent(self.page_id, "tip")["items"], [])

    def teardown(self):
        super().teardown()
        if hasattr(self, "dashboard"):
            self.dashboard.update_settings(message=None, sub_spins=None)


class ModeratorsChangePaidFeatures(ModeratorsTestCase):
    """Let a moderator turn the streamer's paid tips, wheel and songs on and off and set their amounts.

    The streamer's PayPal seller is connected, so the money still goes to the streamer, whose
    currency only they can change.
    """

    login = "modpaid"

    def setup(self):
        super().setup()
        self.context.page_with_paypal(self.dashboard)
        self.dashboard.update_settings(songs=None, tips=None, spin=None)
        self.dashboard.update_settings(currency="USD")

    def run(self):
        self.add_moderator()
        settings = self.moderator.moderated_account(self.page_id)["settings"]
        self.assert_equal(
            [settings["payable"], settings["merchantId"], settings["paypal"]], [True, None, False]
        )
        self.assert_equal([settings["songs"], settings["tips"], settings["spin"]], [None, None, None])
        settings = self.moderator.moderated_settings(
            self.page_id,
            currency="EUR",
            songs={"price": 150},
            tips={"amounts": [200, 700]},
            spin={"options": [{"spins": 1, "price": 300}, {"spins": 3, "price": 500}]},
        )
        self.assert_equal(settings["currency"], "USD", "currency unchanged by a moderator")
        self.assert_equal(settings["songs"], {"enabled": True, "price": 150})
        self.assert_equal(settings["tips"], {"enabled": True, "amounts": [200, 700]})
        self.assert_equal(
            settings["spin"],
            {"enabled": True, "options": [{"spins": 1, "price": 300}, {"spins": 3, "price": 500}]},
        )
        self.assert_equal([settings["payable"], settings["merchantId"]], [True, None])
        own = self.dashboard.account()["settings"]
        for field in ["currency", "songs", "tips", "spin"]:
            self.assert_equal(own[field], settings[field], f"{field} saved by a moderator")
        paid_songs = self.viewer.paid_songs()
        assert paid_songs is not None
        tips = self.viewer.tips()
        assert tips is not None
        self.assert_equal(paid_songs["price"], 150)
        self.assert_equal(tips["amounts"], [200, 700])
        self.assert_equal(paid_songs["merchantId"], own["merchantId"])
        off = {
            "songs": {**settings["songs"], "enabled": False},
            "tips": {"enabled": True, "amounts": [500]},
            "spin": {**settings["spin"], "enabled": False},
        }
        settings = self.moderator.moderated_settings(self.page_id, **off)
        self.assert_equal(
            {field: settings[field] for field in off}, off, "turned off features keep their values"
        )
        own = self.dashboard.account()["settings"]
        self.assert_equal({field: own[field] for field in off}, off)
        self.assert_equal(self.viewer.paid_songs(), None)
        tips = self.viewer.tips()
        assert tips is not None
        self.assert_equal(tips["amounts"], [500])
        self.dashboard.remove_moderator("twitch", self.moderator_id)
        self.assert_backend_error(
            403, lambda: self.moderator.moderated_settings(self.page_id, tips=None), NOT_MODERATOR
        )
        self.assert_equal(self.dashboard.account()["settings"]["tips"], {"enabled": True, "amounts": [500]})

    def teardown(self):
        super().teardown()
        if hasattr(self, "dashboard"):
            self.dashboard.update_settings(songs=None, tips=None, spin=None)
            self.dashboard.update_settings(currency="USD")


class ModeratorsAddSongs(ModeratorsTestCase):
    """Let a moderator add songs up to the longest song without paying, listed as theirs for both of them.

    The song is 200 seconds long with the fake Moblin and 225 with Apple Music, so between the limits of 3 and 4 minutes.
    """

    login = "modsongs"

    def run(self):
        self.skip_if_missing_capability(Capability.APPLE_MUSIC)
        self.add_moderator()
        self.dashboard.update_settings(max_song_length={"minutes": 3})
        self.assert_equal(self.viewer.get(f"/pages/{self.page_id}/songs")["maxSongMinutes"], 3)
        self.connect_moblin()
        self.moblin.clear_music()
        self.assert_backend_error(
            400,
            lambda: self.moderator.add_free_song(self.page_id, "Take on me"),
            "Paste the Apple Music share link of a song.",
        )
        self.assert_backend_error(
            422,
            lambda: self.moderator.add_free_song(self.page_id, MISSING_SONG),
            "That song is not available on the streamer's Apple Music.",
        )
        self.assert_backend_error(
            422,
            lambda: self.moderator.add_free_song(self.page_id, SONG),
            "The streamer only takes songs up to 3 minutes long.",
        )
        self.dashboard.update_settings(max_song_length={"minutes": 4})
        start = self.moblin.backend_requests_position()
        self.moderator.add_free_song(self.page_id, SONG, " For the drive ")
        add = self.moblin.wait_for_new_backend_request("add", start)
        self.assert_equal(
            [add.body["title"], add.body["user"], add.body.get("message")],
            [SONG_LINK, self.moderator_login, "For the drive"],
        )
        self.wait_for_queue([self.moderator_login])
        self.wait_until(
            lambda: (
                self.moderator.moderated_recent(self.page_id, "song")["items"][0]["status"] == "delivered"
            ),
            "the song to be in Moblin",
        )
        for item in [
            self.moderator.moderated_recent(self.page_id, "song")["items"][0],
            self.dashboard.recent("song")["items"][0],
        ]:
            self.assert_equal(
                [
                    item["user"],
                    item["amount"],
                    item.get("moderator"),
                    item.get("provider"),
                    item.get("message"),
                ],
                [self.moderator_login, 0, True, None, "For the drive"],
            )
        for kind in ["tip", "spin"]:
            self.assert_equal(self.moderator.moderated_recent(self.page_id, kind)["more"], False)
        self.assert_backend_error(
            400, lambda: self.moderator.moderated_recent(self.page_id, "bogus"), "Unknown kind."
        )

    def teardown(self):
        super().teardown()
        if hasattr(self, "dashboard"):
            self.dashboard.update_settings(max_song_length=None)
        if self.context.moblin is not None and self.moblin.has_capability(Capability.APPLE_MUSIC):
            self.moblin.clear_music()


class ModeratorsSongRequestsOff(ModeratorsTestCase):
    """Refuse a moderator's songs while song requests are off in Moblin, like everyone else's."""

    login = "modoff"

    def run(self):
        self.skip_if_missing_capability(Capability.APPLE_MUSIC)
        if not self.moblin.is_fake():
            self.skip_if_not_interactive()
        self.add_moderator()
        self.connect_moblin()
        self.moblin.clear_music()
        self.moblin.change_song_requests(False)
        self.wait_until(lambda: not self.viewer.queue()["songRequests"], "song requests to be off")
        self.assert_backend_error(
            409,
            lambda: self.moderator.add_free_song(self.page_id, SONG),
            "The streamer is not taking song requests right now.",
        )
        queue = self.moderator.moderate_music(self.page_id, "pause")
        self.assert_equal(queue["songRequests"], False)
        self.moblin.change_song_requests(True)
        self.wait_until(lambda: self.viewer.queue()["songRequests"], "song requests to be on again")
        self.moderator.add_free_song(self.page_id, SONG)
        self.wait_for_queue([self.moderator_login])

    def teardown(self):
        super().teardown()
        if self.context.moblin is not None and self.moblin.is_fake():
            self.moblin.change_song_requests(True)
            self.moblin.clear_music()


class ModeratorsKeptOnRestart(ModeratorsTestCase):
    """Keep who moderates whom when the backend restarts."""

    login = "modrestart"

    def run(self):
        self.add_moderator()
        self.backend.restart()
        self.assert_equal([moderator["id"] for moderator in self.listed_moderators()], [self.moderator_id])
        self.assert_equal([channel["id"] for channel in self.moderating()], [self.page_id])
        self.assert_equal(self.moderator.moderated_recent(self.page_id, "song")["more"], False)


class ModeratorsCounted(ModeratorsTestCase):
    """Count moderators' songs and queue changes, and why they were refused, for the admin page."""

    login = "modcount"

    def run(self):
        self.skip_if_missing_capability(Capability.APPLE_MUSIC)
        admin = self.sign_in("admin")

        def totals() -> dict[str, int]:
            return {counter["name"]: counter["total"] for counter in admin.admin_stats()["counters"]}

        self.add_moderator()
        self.connect_moblin()
        self.moblin.clear_music()
        before = totals()
        self.moderator.moderate_music(self.page_id, "play")
        self.assert_backend_error(400, lambda: self.moderator.moderate_music(self.page_id, "shuffle"))
        self.moderator.add_free_song(self.page_id, SONG)
        self.assert_backend_error(422, lambda: self.moderator.add_free_song(self.page_id, MISSING_SONG))
        self.dashboard.remove_moderator("twitch", self.moderator_id)
        self.assert_backend_error(403, lambda: self.moderator.moderate_music(self.page_id, "play"))
        self.assert_backend_error(403, lambda: self.moderator.add_free_song(self.page_id, SONG))
        after = totals()
        for name in [
            "moderatorMusic",
            "moderatorMusicFailed:invalid-command",
            "moderatorMusicFailed:not-moderator",
            "modSongs",
            "modSongsFailed:song-not-found",
            "modSongsFailed:not-moderator",
        ]:
            self.assert_equal(after.get(name, 0) - before.get(name, 0), 1, name)

    def teardown(self):
        super().teardown()
        if self.context.moblin is not None and self.moblin.has_capability(Capability.APPLE_MUSIC):
            self.moblin.clear_music()


def tests(context: Context):
    return [
        ModeratorsAdded(context),
        ModeratorsManageTheQueue(context),
        ModeratorsUseTheStreamersDashboard(context),
        ModeratorsAddSongs(context),
        ModeratorsSongRequestsOff(context),
        ModeratorsKeptOnRestart(context),
        ModeratorsCounted(context),
    ]
