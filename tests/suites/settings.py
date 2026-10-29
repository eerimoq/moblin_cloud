from ..utils.context import Context
from ..utils.fake_twitch import user_id_of
from ..utils.test_case import TestCase


class SettingsTestCase(TestCase):
    login = "settingsbase"

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.dashboard = self.sign_in(self.login)
        self.settings = self.context.page_with_paypal(self.dashboard)
        self.page_id = self.settings["id"]
        self.viewer = self.frontend.viewer(self.page_id)
        self.dashboard.change_settings(tips=None, spin=None, effects=None)
        self.dashboard.change_settings(currency="USD")


class SettingsCurrencyLocked(SettingsTestCase):
    """Refuse changing the currency while tips, the wheel or effects are on, so no amount silently changes currency."""

    login = "settingscurrency"

    def run(self):
        features: list[dict[str, dict]] = [
            {"tips": {"amounts": [100]}},
            {"spin": {"options": [{"spins": 1, "price": 100}]}},
            {"effects": {"price": 100}},
        ]
        for feature in features:
            self.dashboard.change_settings(**feature)
            self.assert_backend_error(409, lambda: self.dashboard.change_settings(currency="EUR"))
            self.assert_equal(self.dashboard.account()["settings"]["currency"], "USD", f"{feature}")
            off = {name: {**value, "enabled": False} for name, value in feature.items()}
            self.dashboard.change_settings(**off)
            settings = self.dashboard.change_settings(currency="EUR")
            self.assert_equal(settings["currency"], "EUR", f"changed while {feature} is off")
            for name, value in feature.items():
                self.assert_equal(settings[name], {**value, "enabled": False}, "kept while off")
            on = self.dashboard.change_settings(**feature)
            self.assert_equal(on["currency"], "EUR")
            self.dashboard.change_settings(tips=None, spin=None, effects=None)
            self.dashboard.change_settings(currency="USD")


class SettingsCurrencyWithFeatureOn(SettingsTestCase):
    """Let the streamer pick the currency in the same save that turns a paid feature on."""

    login = "settingscurrencyon"

    def run(self):
        settings = self.dashboard.change_settings(
            currency="SEK", tips={"amounts": [2000]}, effects={"price": 500}
        )
        self.assert_equal(settings["currency"], "SEK")
        tips = self.viewer.tips()
        assert tips is not None
        self.assert_equal(tips["currency"], "SEK")
        self.assert_equal(self.viewer.effects()["effects"]["currency"], "SEK")
        order_id = self.viewer.order_effect("fart", 500)
        unit = self.context.paypal.order(order_id, self.settings["merchantId"])["purchase_units"][0]
        self.assert_equal(unit["amount"], {"currency_code": "SEK", "value": "5.00"})

    def teardown(self):
        if hasattr(self, "dashboard"):
            self.dashboard.change_settings(tips=None, spin=None, effects=None)
            self.dashboard.change_settings(currency="USD")


class SettingsModeratorKeepsCurrency(SettingsTestCase):
    """Keep the page's currency when a moderator saves, also while all paid features are off."""

    login = "settingsmodcurrency"

    def run(self):
        moderator = self.sign_in("settingsmod")
        self.dashboard.add_moderator("twitch", "settingsmod")
        self.dashboard.set_moderator_edit_tips("twitch", user_id_of("settingsmod"), True)
        try:
            settings = moderator.post(
                f"/account/pages/{self.page_id}/settings",
                {
                    "currency": "EUR",
                    "tips": None,
                    "spin": None,
                    "effects": {"price": 300},
                    "message": None,
                    "accent": None,
                    "font": None,
                    "subSpins": None,
                    "subEffects": None,
                },
            )
            self.assert_equal(
                [settings["currency"], settings["effects"]], ["USD", {"enabled": True, "price": 300}]
            )
            self.assert_equal(self.dashboard.account()["settings"]["currency"], "USD")
            self.assert_equal(self.viewer.effects()["effects"]["currency"], "USD")
        finally:
            self.dashboard.remove_moderator("twitch", user_id_of("settingsmod"))
            self.dashboard.change_settings(effects=None)


class SettingsAccent(TestCase):
    """Let premium streamers pick any #rrggbb accent color and others only the fixed ones."""

    def setup(self):
        self.skip_if_real_twitch_sign_in()

    def run(self):
        premium = self.sign_in("admin")
        settings = premium.change_settings(accent="#12ab34")
        self.assert_equal(settings["accent"], "#12ab34")
        self.assert_equal(self.frontend.viewer(settings["id"]).profile()["accent"], "#12ab34")
        for invalid in ["#12ab3g", "12ab34", "#12ab345", "#fff", "#12ab34;color:red", "black"]:
            self.assert_refused(lambda invalid=invalid: premium.change_settings(accent=invalid))
        self.assert_equal(premium.account()["settings"]["accent"], "#12ab34")
        premium.change_settings(accent=None)
        normal = self.sign_in("normalaccent")
        self.assert_refused(lambda: normal.change_settings(accent="#12ab34"))
        self.assert_equal(normal.account()["settings"]["accent"], None)
        self.assert_equal(normal.change_settings(accent="purple")["accent"], "purple")
        normal.change_settings(accent=None)


class SettingsSubscriberTiers(TestCase):
    """Take 0 to 100 subscriber spins and effects per tier, at least one above 0, and keep them while off."""

    def setup(self):
        self.skip_if_real_twitch_sign_in()

    def run(self):
        dashboard = self.sign_in("settingstiers")
        for name in ["subSpins", "subEffects"]:
            self.assert_equal(
                dashboard.change_settings(**{name: {"tier1": 100, "tier2": 0, "tier3": 0}})[name],
                {"enabled": True, "tier1": 100, "tier2": 0, "tier3": 0},
            )
            for invalid in [{"tier1": 0, "tier2": 0, "tier3": 0}, {"tier1": 101, "tier2": 1, "tier3": 1}]:
                self.assert_refused(
                    lambda invalid=invalid, name=name: dashboard.change_settings(**{name: invalid})
                )
            self.assert_refused(
                lambda name=name: dashboard.change_settings(**{name: {"tier1": -1, "tier2": 1, "tier3": 1}})
            )
            off = dashboard.change_settings(**{name: {"enabled": False, "tier1": 5, "tier2": 0, "tier3": 0}})
            self.assert_equal(off[name], {"enabled": False, "tier1": 5, "tier2": 0, "tier3": 0})
            self.assert_equal(
                self.frontend.viewer(off["id"]).effects()["subEffects"]
                if name == "subEffects"
                else self.frontend.viewer(off["id"]).spin()["subSpins"],
                None,
            )
            dashboard.change_settings(**{name: None})


def tests(context: Context):
    return [
        SettingsCurrencyLocked(context),
        SettingsCurrencyWithFeatureOn(context),
        SettingsModeratorKeepsCurrency(context),
        SettingsAccent(context),
        SettingsSubscriberTiers(context),
    ]
