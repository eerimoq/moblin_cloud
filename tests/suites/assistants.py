from ..utils.context import Context
from ..utils.fake_twitch import user_id_of
from ..utils.frontend import Dashboard
from ..utils.moblin import Moblin
from ..utils.test_case import TestCase

NOT_ASSISTANT = "Only the streamer and the assistants they turned on can do this."
NO_EDIT_TIPS = "The streamer has not let you edit tips."


class AssistantsTestCase(TestCase):
    """A streamer and a made-up Twitch user they add as an assistant by username.

    The fake Twitch resolves any login to a user, standing in for Twitch's user lookup. Each test has
    its own streamer, since test sends are limited per page and minute.
    """

    login = "streamer"

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.dashboard = self.sign_in(self.login)
        self.page_id = self.dashboard.page_id()
        self.viewer = self.frontend.viewer(self.page_id)
        self.assistant_login = f"mod_{self.login}"
        self.assistant = self.sign_in(self.assistant_login)
        self.assistant_id = user_id_of(self.assistant_login)

    def teardown(self):
        if hasattr(self, "assistant_id") and self.listed_assistants():
            self.dashboard.remove_assistant("twitch", self.assistant_id)

    def add_assistant(self):
        account = self.dashboard.add_assistant("twitch", self.assistant_login)
        self.assert_equal([assistant["id"] for assistant in account["twitchAssistants"]], [self.assistant_id])

    @property
    def moblin(self) -> Moblin:
        assert self.context.moblin is not None
        return self.context.moblin

    def listed_assistants(self) -> list[dict]:
        return [
            assistant
            for assistant in self.dashboard.account()["twitchAssistants"]
            if assistant["login"] == self.assistant_login
        ]

    def assisting(self) -> list[dict]:
        return [channel for channel in self.assistant.account()["assisting"] if channel["id"] == self.page_id]

    def connect_moblin(self):
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key())
        self.wait_until(lambda: self.viewer.spin()["connected"], "Moblin to be connected")


class AssistantsAdded(AssistantsTestCase):
    """Let an assistant help only once the streamer added them by username, and not once removed."""

    login = "modon"

    def run(self):
        self.assert_equal(self.listed_assistants(), [])
        self.assert_equal(self.assisting(), [])
        self.assert_backend_error(403, lambda: self.assistant.assisted_test_tip(self.page_id), NOT_ASSISTANT)
        self.assert_backend_error(
            403, lambda: self.assistant.assisted_recent(self.page_id, "tip"), NOT_ASSISTANT
        )
        self.assert_backend_error(
            404,
            lambda: self.dashboard.add_assistant("twitch", "nobody"),
            "There is no Twitch user named nobody.",
        )
        self.assert_backend_error(
            400, lambda: self.dashboard.add_assistant("twitch", "bad name"), "Enter a Twitch username."
        )
        self.assert_backend_error(
            409,
            lambda: self.dashboard.add_assistant("twitch", self.login),
            "That is your own Twitch account.",
        )
        self.assert_backend_error(
            404, lambda: self.dashboard.remove_assistant("twitch", "1"), "no such assistant"
        )
        self.assert_backend_error(
            409, lambda: self.dashboard.add_assistant("kick", "someone"), "Link Kick first."
        )
        self.add_assistant()
        self.assert_equal(
            self.listed_assistants(),
            [
                {
                    "id": self.assistant_id,
                    "login": self.assistant_login,
                    "name": self.assistant_login,
                    "avatar": None,
                    "preview": False,
                    "goLive": False,
                    "editTips": False,
                    "remoteControl": "none",
                }
            ],
        )
        streamer = self.dashboard.account()
        self.assert_equal(
            self.assisting(),
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
            self.assert_equal(self.assistant.assisted_recent(self.page_id, kind)["more"], False)
        self.assert_backend_error(
            400, lambda: self.assistant.assisted_recent(self.page_id, "bogus"), "Unknown kind."
        )
        self.assert_backend_error(
            401, lambda: Dashboard(self.frontend.url, "garbage").assisted_recent(self.page_id, "tip")
        )
        other = self.sign_in("otherstreamer")
        self.assert_backend_error(403, lambda: other.assisted_test_tip(self.page_id), NOT_ASSISTANT)
        self.assert_backend_error(403, lambda: other.assisted_recent(self.page_id, "tip"), NOT_ASSISTANT)
        account = self.dashboard.remove_assistant("twitch", self.assistant_id)
        self.assert_equal(account["twitchAssistants"], [])
        self.assert_equal(self.listed_assistants(), [])
        self.assert_equal(self.assisting(), [])
        self.assert_backend_error(403, lambda: self.assistant.assisted_test_tip(self.page_id), NOT_ASSISTANT)


class AssistantsUseTheStreamersDashboard(AssistantsTestCase):
    """Show an assistant the streamer's dashboard without payments, letting them change the rest only once the streamer lets them edit tips.

    The streamer has no PayPal or Stripe, so the paid features can't be turned on by anyone.
    """

    login = "moddash"

    def run(self):
        self.dashboard.update_settings(message="Mine", sub_spins=None)
        for call in [
            lambda: self.assistant.assisted_account(self.page_id),
            lambda: self.assistant.assisted_settings(self.page_id, message="Theirs"),
            lambda: self.assistant.assisted_test_tip(self.page_id),
            lambda: self.assistant.assisted_test_spin(self.page_id),
            lambda: self.assistant.assisted_remove_image(self.page_id),
        ]:
            self.assert_backend_error(403, call, NOT_ASSISTANT)
        self.add_assistant()
        own = self.dashboard.account()
        account = self.assistant.assisted_account(self.page_id)
        self.assert_equal(
            [
                account["name"],
                account["twitch"],
                account["key"],
                account["devices"],
                account["twitchAssistants"],
                account["assisting"],
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
        self.assert_equal(account["editTips"], False)
        for call in [
            lambda: self.assistant.assisted_settings(self.page_id, message="Theirs"),
            lambda: self.assistant.assisted_remove_image(self.page_id),
        ]:
            self.assert_backend_error(403, call, NO_EDIT_TIPS)
        self.assert_equal(self.dashboard.account()["settings"]["message"], "Mine")
        account = self.dashboard.set_assistant_edit_tips("twitch", self.assistant_id, True)
        self.assert_equal([entry["editTips"] for entry in account["twitchAssistants"]], [True])
        self.assert_equal(self.assistant.assisted_account(self.page_id)["editTips"], True)
        for paid in [
            {"tips": {"amounts": [100]}},
            {"spin": {"options": [{"spins": 1, "price": 100}]}},
        ]:
            self.assert_backend_error(
                409,
                lambda paid=paid: self.assistant.assisted_settings(self.page_id, message="Theirs", **paid),
                "connect PayPal or Stripe first",
            )
        settings = self.assistant.assisted_settings(
            self.page_id,
            message="Theirs",
            currency="EUR" if own["settings"]["currency"] != "EUR" else "USD",
            sub_spins={"tier1": 1, "tier2": 2, "tier3": 3},
        )
        self.assert_equal(settings["message"], "Theirs")
        self.assert_equal(settings["subSpins"], {"enabled": True, "tier1": 1, "tier2": 2, "tier3": 3})
        self.assert_equal(settings["merchantId"], None)
        self.assert_equal(
            settings["currency"], own["settings"]["currency"], "currency unchanged by an assistant"
        )
        own = self.dashboard.account()
        self.assert_equal(own["settings"]["message"], "Theirs")
        self.assert_equal(own["settings"]["message"], "Theirs")
        self.assert_equal(own["settings"]["subSpins"], {"enabled": True, "tier1": 1, "tier2": 2, "tier3": 3})
        self.assert_backend_error(
            409, lambda: self.assistant.assisted_test_spin(self.page_id), "Connect Moblin first."
        )
        self.assistant.assisted_remove_image(self.page_id)
        self.connect_moblin()
        start = self.moblin.backend_requests_position()
        self.assistant.assisted_test_tip(self.page_id, " Checking ")
        tip = self.moblin.wait_for_new_backend_request("message", start)
        self.assert_equal(
            [tip.body["name"], tip.body["message"], tip.body["amount"]],
            [self.assistant_login, "Checking", "5.00"],
        )
        self.assert_equal(self.assistant.assisted_recent(self.page_id, "tip")["items"], [])

    def teardown(self):
        super().teardown()
        if hasattr(self, "dashboard"):
            self.dashboard.update_settings(message=None, sub_spins=None)


class AssistantsChangePaidFeatures(AssistantsTestCase):
    """Let an assistant turn the streamer's paid tips and wheel on and off and set their amounts.

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
        self.add_assistant()
        self.dashboard.set_assistant_edit_tips("twitch", self.assistant_id, True)
        settings = self.assistant.assisted_account(self.page_id)["settings"]
        self.assert_equal(
            [settings["payable"], settings["merchantId"], settings["paypal"]], [True, None, False]
        )
        self.assert_equal([settings["tips"], settings["spin"]], [None, None])
        settings = self.assistant.assisted_settings(
            self.page_id,
            currency="EUR",
            tips={"amounts": [200, 700]},
            spin={"options": [{"spins": 1, "price": 300}, {"spins": 3, "price": 500}]},
        )
        self.assert_equal(settings["currency"], "USD", "currency unchanged by an assistant")
        self.assert_equal(settings["tips"], {"enabled": True, "amounts": [200, 700]})
        self.assert_equal(
            settings["spin"],
            {"enabled": True, "options": [{"spins": 1, "price": 300}, {"spins": 3, "price": 500}]},
        )
        self.assert_equal([settings["payable"], settings["merchantId"]], [True, None])
        own = self.dashboard.account()["settings"]
        for field in ["currency", "tips", "spin"]:
            self.assert_equal(own[field], settings[field], f"{field} saved by an assistant")
        tips = self.viewer.tips()
        assert tips is not None
        self.assert_equal(tips["amounts"], [200, 700])
        self.assert_equal(tips["merchantId"], own["merchantId"])
        off = {
            "tips": {"enabled": True, "amounts": [500]},
            "spin": {**settings["spin"], "enabled": False},
        }
        settings = self.assistant.assisted_settings(self.page_id, **off)
        self.assert_equal(
            {field: settings[field] for field in off}, off, "turned off features keep their values"
        )
        own = self.dashboard.account()["settings"]
        self.assert_equal({field: own[field] for field in off}, off)
        self.assert_equal(self.viewer.spin()["spin"], None)
        tips = self.viewer.tips()
        assert tips is not None
        self.assert_equal(tips["amounts"], [500])
        self.dashboard.remove_assistant("twitch", self.assistant_id)
        self.assert_backend_error(
            403, lambda: self.assistant.assisted_settings(self.page_id, tips=None), NOT_ASSISTANT
        )
        self.assert_equal(self.dashboard.account()["settings"]["tips"], {"enabled": True, "amounts": [500]})

    def teardown(self):
        super().teardown()
        if hasattr(self, "dashboard"):
            self.dashboard.update_settings(tips=None, spin=None)
            self.dashboard.update_settings(currency="USD")


class AssistantsKeptOnRestart(AssistantsTestCase):
    """Keep who assists whom when the backend restarts."""

    login = "modrestart"

    def run(self):
        self.add_assistant()
        self.backend.restart()
        self.assert_equal([assistant["id"] for assistant in self.listed_assistants()], [self.assistant_id])
        self.assert_equal([channel["id"] for channel in self.assisting()], [self.page_id])
        self.assert_equal(self.assistant.assisted_recent(self.page_id, "tip")["more"], False)


def tests(context: Context):
    return [
        AssistantsAdded(context),
        AssistantsUseTheStreamersDashboard(context),
        AssistantsKeptOnRestart(context),
    ]
