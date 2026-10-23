from ..utils.context import Context
from ..utils.moblin import Moblin
from ..utils.test_case import TestCase
from ..utils.website import Dashboard

NOT_MODERATOR = "Only the streamer and the moderators they turned on can do this."


class ModeratorsTestCase(TestCase):
    """A streamer and a made-up Twitch user they add as a moderator by username.

    The fake Twitch resolves any login to a user, standing in for Twitch's user lookup. Each test has
    its own streamer, since test sends are limited per page and minute.
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


class ModeratorsAdded(ModeratorsTestCase):
    """Let a moderator help only once the streamer added them by username, and not once removed."""

    login = "modon"

    def run(self):
        self.assert_equal(self.listed_moderators(), [])
        self.assert_equal(self.moderating(), [])
        self.assert_backend_error(403, lambda: self.moderator.moderated_test_tip(self.page_id), NOT_MODERATOR)
        self.assert_backend_error(
            403, lambda: self.moderator.moderated_recent(self.page_id, "tip"), NOT_MODERATOR
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
        for kind in ["tip", "spin"]:
            self.assert_equal(self.moderator.moderated_recent(self.page_id, kind)["more"], False)
        self.assert_backend_error(
            400, lambda: self.moderator.moderated_recent(self.page_id, "bogus"), "Unknown kind."
        )
        self.assert_backend_error(
            401, lambda: Dashboard(self.website.url, "garbage").moderated_recent(self.page_id, "tip")
        )
        other = self.sign_in("otherstreamer")
        self.assert_backend_error(403, lambda: other.moderated_test_tip(self.page_id), NOT_MODERATOR)
        self.assert_backend_error(403, lambda: other.moderated_recent(self.page_id, "tip"), NOT_MODERATOR)
        account = self.dashboard.remove_moderator("twitch", self.moderator_id)
        self.assert_equal(account["twitchModerators"], [])
        self.assert_equal(self.listed_moderators(), [])
        self.assert_equal(self.moderating(), [])
        self.assert_backend_error(403, lambda: self.moderator.moderated_test_tip(self.page_id), NOT_MODERATOR)


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
    """Let a moderator turn the streamer's paid tips and wheel on and off and set their amounts.

    The streamer's PayPal seller is connected, so the money still goes to the streamer, whose
    currency only they can change.
    """

    login = "modpaid"

    def setup(self):
        super().setup()
        self.context.page_with_paypal(self.dashboard)
        self.dashboard.update_settings(tips=None, spin=None)
        self.dashboard.update_settings(currency="USD")

    def run(self):
        self.add_moderator()
        settings = self.moderator.moderated_account(self.page_id)["settings"]
        self.assert_equal(
            [settings["payable"], settings["merchantId"], settings["paypal"]], [True, None, False]
        )
        self.assert_equal([settings["tips"], settings["spin"]], [None, None])
        settings = self.moderator.moderated_settings(
            self.page_id,
            currency="EUR",
            tips={"amounts": [200, 700]},
            spin={"options": [{"spins": 1, "price": 300}, {"spins": 3, "price": 500}]},
        )
        self.assert_equal(settings["currency"], "USD", "currency unchanged by a moderator")
        self.assert_equal(settings["tips"], {"enabled": True, "amounts": [200, 700]})
        self.assert_equal(
            settings["spin"],
            {"enabled": True, "options": [{"spins": 1, "price": 300}, {"spins": 3, "price": 500}]},
        )
        self.assert_equal([settings["payable"], settings["merchantId"]], [True, None])
        own = self.dashboard.account()["settings"]
        for field in ["currency", "tips", "spin"]:
            self.assert_equal(own[field], settings[field], f"{field} saved by a moderator")
        tips = self.viewer.tips()
        assert tips is not None
        self.assert_equal(tips["amounts"], [200, 700])
        self.assert_equal(tips["merchantId"], own["merchantId"])
        off = {
            "tips": {"enabled": True, "amounts": [500]},
            "spin": {**settings["spin"], "enabled": False},
        }
        settings = self.moderator.moderated_settings(self.page_id, **off)
        self.assert_equal(
            {field: settings[field] for field in off}, off, "turned off features keep their values"
        )
        own = self.dashboard.account()["settings"]
        self.assert_equal({field: own[field] for field in off}, off)
        self.assert_equal(self.viewer.spin()["spin"], None)
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
            self.dashboard.update_settings(tips=None, spin=None)
            self.dashboard.update_settings(currency="USD")


class ModeratorsKeptOnRestart(ModeratorsTestCase):
    """Keep who moderates whom when the backend restarts."""

    login = "modrestart"

    def run(self):
        self.add_moderator()
        self.backend.restart()
        self.assert_equal([moderator["id"] for moderator in self.listed_moderators()], [self.moderator_id])
        self.assert_equal([channel["id"] for channel in self.moderating()], [self.page_id])
        self.assert_equal(self.moderator.moderated_recent(self.page_id, "tip")["more"], False)


def tests(context: Context):
    return [
        ModeratorsAdded(context),
        ModeratorsUseTheStreamersDashboard(context),
        ModeratorsKeptOnRestart(context),
    ]
