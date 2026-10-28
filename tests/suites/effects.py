import requests

from ..utils.context import Context
from ..utils.devices import RecordingMoblin
from ..utils.test_case import TestCase

EFFECTS = ["fart", "fart-2", "bad-chili-fart", "perfect-fart"]
WHEEL = ["Dance", "Sing"]


class EffectsTestCase(TestCase):
    """A streamer's page with paid or subscribers' effects, which the fake Moblin devices of the page play."""

    login = "effects"

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.dashboard = self.sign_in(self.login)
        self.page_id = self.dashboard.page_id()
        self.viewer = self.frontend.viewer(self.page_id)

    def device(self, device_id: str = "EFFECTS", wheel: list[str] | None = None) -> RecordingMoblin:
        device = RecordingMoblin(self.backend.url, self.dashboard.moblin_cloud_key(), device_id, wheel)
        device.start()
        self.wait_until(lambda: self.viewer.spin()["connected"], "the device to connect")
        if wheel is not None:
            self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "the wheel to be on the page")
        return device


class EffectsOptions(EffectsTestCase):
    """List the effects a viewer can pick and serve each one's sound so the page can play it."""

    login = "effectoptions"

    def run(self):
        effects = self.viewer.effects()
        self.assert_equal(effects["effects"], None)
        self.assert_equal(effects["subEffects"], None)
        self.assert_equal(effects["options"], [{"id": effect, "kind": "sound"} for effect in EFFECTS])
        bodies = set()
        for effect in EFFECTS:
            response = requests.get(f"{self.backend.url}/effects/{effect}", timeout=10)
            self.assert_equal(response.status_code, 200, effect)
            self.assert_true(response.headers["content-type"].startswith("audio/"), response.headers)
            self.assert_true(len(response.content) > 1000, effect)
            bodies.add(response.content)
        self.assert_equal(len(bodies), len(EFFECTS), "every effect has its own sound")
        for unknown in ["nope", "FART", "fart.mp3", "..%2Ffart"]:
            response = requests.get(f"{self.backend.url}/effects/{unknown}", timeout=10)
            self.assert_equal(response.status_code, 404, unknown)


class EffectsNeedPayments(EffectsTestCase):
    """Refuse to turn paid effects on before PayPal or Stripe is connected, but not subscribers' effects."""

    login = "effectsneedpay"

    def run(self):
        self.assert_backend_error(
            409,
            lambda: self.dashboard.change_settings(effects={"price": 100}),
            "connect PayPal or Stripe first",
        )
        self.assert_equal(self.viewer.effects()["effects"], None)
        self.assert_backend_error(400, lambda: self.viewer.order_effect("fart", 100))
        settings = self.dashboard.change_settings(subEffects={"tier1": 1, "tier2": 0, "tier3": 0})
        self.assert_equal(settings["subEffects"], {"enabled": True, "tier1": 1, "tier2": 0, "tier3": 0})
        self.assert_equal(self.viewer.effects()["subEffects"], {"tier1": 1, "tier2": 0, "tier3": 0})
        self.dashboard.change_settings(subEffects=None)
        self.assert_equal(self.viewer.effects()["subEffects"], None)


class EffectsInvalid(EffectsTestCase):
    """Refuse effect prices out of range, and orders of unknown effects, old prices or as another kind."""

    login = "effectsinvalid"

    def run(self):
        self.context.page_with_paypal(self.dashboard)
        for price in [0, 49, 10_000_001]:
            self.assert_backend_error(
                400, lambda price=price: self.dashboard.change_settings(effects={"price": price})
            )
        self.assert_refused(lambda: self.dashboard.change_settings(effects={"price": -100}))
        for price in [50, 10_000_000]:
            self.assert_equal(
                self.dashboard.change_settings(effects={"price": price})["effects"]["price"], price
            )
        settings = self.dashboard.change_settings(effects={"price": 150})
        self.assert_equal(settings["effects"], {"enabled": True, "price": 150})
        effects = self.viewer.effects()["effects"]
        self.assert_equal(
            [effects["price"], effects["currency"], effects["merchantId"]],
            [150, "USD", settings["merchantId"]],
        )
        self.assert_backend_error(400, lambda: self.viewer.order_effect("nope", 150))
        self.assert_backend_error(400, lambda: self.viewer.order_effect("fart", 100))
        self.assert_backend_error(400, lambda: self.viewer.order_effect("fart", 151))
        self.assert_backend_error(
            400,
            lambda: self.viewer.order_effect("fart", 150, message=201 * "x"),
            "The message can be at most 200 characters.",
        )
        order_id = self.viewer.order_effect("fart", 150, message=200 * "x")
        self.context.paypal.pay_with_card(order_id, settings["merchantId"])
        for capture in [self.viewer.capture_tip, self.viewer.capture_spin]:
            self.assert_backend_error(410, lambda capture=capture: capture(order_id))
        self.assert_equal(self.context.paypal.order(order_id, settings["merchantId"])["status"], "APPROVED")
        self.dashboard.change_settings(effects={"price": 150, "enabled": False})
        self.assert_equal(self.viewer.effects()["effects"], None)
        self.assert_equal(self.dashboard.account()["settings"]["effects"], {"enabled": False, "price": 150})
        self.assert_backend_error(400, lambda: self.viewer.order_effect("fart", 150))
        self.dashboard.change_settings(effects=None)


class EffectsPaid(EffectsTestCase):
    """Pay for an effect, which every device of the page plays, and list it in the dashboard."""

    def __init__(self, context: Context, provider: str = "paypal"):
        super().__init__(context, f"{type(self).__name__}Stripe" if provider == "stripe" else None)
        self.provider = provider
        self.login = f"effectspaid{provider}"

    def run(self):
        if self.provider == "stripe":
            settings = self.context.page_with_stripe(self.dashboard)
        else:
            settings = self.context.page_with_paypal(self.dashboard)
        self.dashboard.change_settings(currency="EUR", effects={"price": 300})
        with self.device("FIRST") as first, self.device("SECOND") as second:
            order_id = self.viewer.order_effect(
                "bad-chili-fart", 300, " Farter ", " Enjoy ", provider=self.provider
            )
            if self.provider == "stripe":
                self.context.stripe.pay_with_card(order_id, settings["stripeAccountId"])
                payment = self.context.stripe.payment(order_id, settings["stripeAccountId"])
                self.assert_equal([payment["amount"], payment["currency"]], [300, "eur"])
            else:
                self.context.paypal.pay_with_card(order_id, settings["merchantId"])
                unit = self.context.paypal.order(order_id, settings["merchantId"])["purchase_units"][0]
                self.assert_equal(unit["amount"], {"currency_code": "EUR", "value": "3.00"})
            self.assert_equal(self.viewer.capture_effect(order_id), {})
            self.wait_until(
                lambda: all(device.tip_with_id(order_id) is not None for device in [first, second]),
                "the effect on every device",
            )
            for device in [first, second]:
                tip = device.tip_with_id(order_id)
                assert tip is not None
                self.assert_equal(
                    tip,
                    {
                        "name": "Farter",
                        "message": "Enjoy",
                        "requestId": order_id,
                        "kind": {"effect": {"sound": "Bad chili fart"}},
                    },
                )
            self.assert_equal(self.viewer.capture_effect(order_id), {})
        items = self.dashboard.recent("effect")["items"]
        self.assert_equal([item["id"] for item in items], [order_id])
        item = items[0]
        self.assert_equal(
            [
                item["effect"],
                item["user"],
                item["message"],
                item["amount"],
                item["currency"],
                item["provider"],
            ],
            ["bad-chili-fart", "Farter", "Enjoy", 300, "EUR", self.provider],
        )
        self.assert_equal(item["status"], "delivered")
        self.assert_equal(sorted(device["received"] for device in item["devices"]), [True, True])
        self.assert_equal(self.dashboard.recent("tip")["items"], [])

    def teardown(self):
        if hasattr(self, "dashboard"):
            self.dashboard.change_settings(effects=None)
            self.dashboard.change_settings(currency="USD")


class EffectsTestSend(EffectsTestCase):
    """Send a free test effect from the dashboard, the picked one or the first one, without PayPal or Moblin."""

    login = "effectstest"

    def run(self):
        self.assert_equal(self.dashboard.test_effect("perfect-fart", "Testing"), {})
        self.assert_equal(self.dashboard.test_effect(), {})
        self.assert_backend_error(400, lambda: self.dashboard.test_effect("nope"))
        with self.device() as device:
            self.wait_until(lambda: len(device.kinds("effect")) >= 2, "the test effects to be played")
            effects = device.kinds("effect")
            self.assert_equal(len(effects), 2, "the refused test effect was not sent")
            self.assert_equal([effect["name"] for effect in effects], ["effectstest", "effectstest"])
            sounds = {effect["message"]: effect["kind"]["effect"]["sound"] for effect in effects}
            self.assert_equal(len(set(sounds.values())), 2, f"different sounds {sounds}")
            self.assert_equal(self.dashboard.recent("effect")["items"], [])


class EffectsForSubscribers(EffectsTestCase):
    """Let subscribers play effects without paying, within their tier's effects, apart from their spins."""

    login = "effectssubs"

    def run(self):
        self.dashboard.change_settings(
            subEffects={"tier1": 1, "tier2": 0, "tier3": 2}, subSpins={"tier1": 1, "tier2": 1, "tier3": 1}
        )
        tier1 = self.frontend.subscriber(self.page_id, "effectfan1")
        self.assert_equal(
            tier1.effect_allowance(),
            {"platform": "twitch", "name": "effectfan1", "tier": 1, "allowance": 1, "left": 1},
        )
        self.assert_backend_error(400, lambda: tier1.play_effect("nope"))
        self.assert_equal(tier1.effect_allowance()["left"], 1, "an unknown effect uses nothing")
        self.assert_equal(tier1.play_effect("fart-2", " Listen "), {"left": 0})
        self.assert_backend_error(409, lambda: tier1.play_effect("fart"))
        self.assert_equal(tier1.spins()["left"], 1, "the effect did not use the spin")
        self.assert_backend_error(
            403, lambda: self.frontend.subscriber(self.page_id, "effectfan2").play_effect("fart")
        )
        self.assert_backend_error(
            403, lambda: self.frontend.subscriber(self.page_id, "effectfanviewer").play_effect("fart")
        )
        tier3 = self.frontend.subscriber(self.page_id, "effectfan3")
        self.assert_equal(tier3.play_effect("fart"), {"left": 1})
        with self.device() as device:
            self.wait_until(lambda: len(device.kinds("effect")) == 2, "the effects to be played")
            first = next(effect for effect in device.kinds("effect") if effect["name"] == "effectfan1")
            self.assert_equal(first["message"], "Listen")
            self.wait_until(
                lambda: all(
                    item["status"] == "delivered" for item in self.dashboard.recent("effect")["items"]
                ),
                "the effects to be delivered",
            )
        items = self.dashboard.recent("effect")["items"]
        self.assert_equal(
            [(item["user"], item.get("tier"), item["effect"]) for item in items],
            [("effectfan3", 3, "fart"), ("effectfan1", 1, "fart-2")],
        )
        self.assert_true(all("provider" not in item for item in items), "subscribers paid with no provider")
        self.dashboard.change_settings(subEffects=None)
        self.assert_backend_error(400, lambda: tier3.play_effect("fart"))

    def teardown(self):
        if hasattr(self, "dashboard"):
            self.dashboard.change_settings(subEffects=None, subSpins=None)


def tests(context: Context):
    return [
        EffectsOptions(context),
        EffectsNeedPayments(context),
        EffectsInvalid(context),
        EffectsPaid(context, "paypal"),
        EffectsPaid(context, "stripe"),
        EffectsTestSend(context),
        EffectsForSubscribers(context),
    ]
