import uuid

import requests

from ..utils.context import Context
from ..utils.devices import RawDevice
from ..utils.devices import refused
from ..utils.fake_kick import user_id_of as kick_user_id_of
from ..utils.fake_twitch import user_id_of
from ..utils.test_case import TestCase
from .images import image


class LifecycleTestCase(TestCase):
    def setup(self):
        self.skip_if_real_twitch_sign_in()


class LifecycleKeptOnRestart(LifecycleTestCase):
    """Keep everything a streamer set up when the backend restarts: identities, devices, assistants, look and features."""

    def run(self):
        dashboard = self.sign_in("admin")
        settings = self.context.page_with_paypal(dashboard)
        page_id = settings["id"]
        if dashboard.account()["kick"] is None:
            dashboard.link(self.frontend.link_token("kick", "adminkick"))
        dashboard.change_settings(
            effects={"price": 250},
            tips={"enabled": False, "amounts": [300]},
            subEffects={"tier1": 2, "tier2": 0, "tier3": 1},
            accent="#abcdef",
            font="serif",
            message="Kept",
        )
        dashboard.upload_image(image(1500, 500))
        dashboard.add_assistant("twitch", "keptmod")
        dashboard.add_assistant("kick", "keptkickmod")
        dashboard.set_assistant_preview("kick", kick_user_id_of("keptkickmod"), True)
        device = RawDevice(self.backend.port, dashboard.moblin_cloud_key(), "KEPT")
        device.event({"icon": {"id": "AppIconKing"}})
        dashboard.rename_device("KEPT", "Kept phone")
        viewer = self.frontend.viewer(page_id)
        self.wait_until(lambda: viewer.profile()["icon"] == "king", "the icon")
        device.close()
        self.wait_until(lambda: not dashboard.account()["devices"][0]["connected"], "the device to leave")
        before = dashboard.account()
        profile = viewer.profile()
        effects = viewer.effects()
        addresses = requests.get(
            f"{self.backend.url}/pages/addresses",
            params={"logins": "twitch:admin,kick:adminkick"},
            timeout=10,
        ).json()
        self.backend.restart()
        self.assert_equal(dashboard.account(), before)
        self.assert_equal(viewer.profile(), profile)
        self.assert_equal(viewer.effects(), effects)
        self.assert_equal(
            requests.get(
                f"{self.backend.url}/pages/addresses",
                params={"logins": "twitch:admin,kick:adminkick"},
                timeout=10,
            ).json(),
            addresses,
        )
        assistant = self.frontend.fake_sign_in("keptkickmod", "kick")
        self.assert_equal(assistant.assisted_account(page_id)["preview"], True)

    def teardown(self):
        dashboard = self.sign_in("admin")
        dashboard.change_settings(
            effects=None, tips=None, subEffects=None, accent=None, font=None, message=None
        )
        dashboard.remove_image()
        for platform, assistants in [("twitch", "twitchAssistants"), ("kick", "kickAssistants")]:
            for assistant in dashboard.account()[assistants]:
                dashboard.remove_assistant(platform, assistant["id"])


class LifecycleOneProviderLeft(LifecycleTestCase):
    """Keep taking payments with Stripe when the streamer disconnects PayPal, and with PayPal when they disconnect Stripe."""

    def run(self):
        dashboard = self.sign_in("bothproviders")
        self.context.page_with_paypal(dashboard)
        settings = self.context.page_with_stripe(dashboard)
        page_id = settings["id"]
        dashboard.change_settings(tips={"amounts": [100]}, effects={"price": 100})
        viewer = self.frontend.viewer(page_id)
        settings = dashboard.disconnect_paypal()
        self.assert_equal([settings["merchantId"], settings["payable"]], [None, True])
        self.assert_equal(settings["tips"], {"enabled": True, "amounts": [100]})
        tips = viewer.tips()
        self.assert_true(tips is not None, "tips stay on with Stripe")
        assert tips is not None
        self.assert_equal(tips["merchantId"], None)
        self.assert_equal(tips["stripe"]["accountId"], settings["stripeAccountId"])
        self.assert_backend_error(503, lambda: viewer.order_tip(100, provider="paypal"))
        order_id = viewer.order_tip(100, provider="stripe")
        self.assert_true(order_id.startswith("pi_"))
        self.assert_true(viewer.order_effect("fart", 100, provider="stripe").startswith("pi_"))
        self.context.page_with_paypal(dashboard)
        settings = dashboard.disconnect_stripe()
        self.assert_equal(settings["stripeAccountId"], None)
        tips = viewer.tips()
        assert tips is not None
        self.assert_equal(tips["stripe"], None)
        self.assert_backend_error(503, lambda: viewer.order_tip(100, provider="stripe"))
        self.assert_equal(dashboard.disconnect_paypal()["payable"], False)
        self.assert_equal([viewer.tips(), viewer.effects()["effects"]], [None, None])
        dashboard.delete_account()


class LifecycleCaptureAfterDelete(LifecycleTestCase):
    """Take no money for an order of a page whose streamer deleted their account before the viewer paid."""

    def run(self):
        for provider in ["paypal", "stripe"]:
            dashboard = self.sign_in(f"deletedpage{provider}")
            if provider == "stripe":
                settings = self.context.page_with_stripe(dashboard)
            else:
                settings = self.context.page_with_paypal(dashboard)
            dashboard.change_settings(tips={"amounts": [100]})
            viewer = self.frontend.viewer(settings["id"])
            order_id = viewer.order_tip(100, provider=provider)
            if provider == "stripe":
                self.context.stripe.pay_with_card(order_id, settings["stripeAccountId"])
            else:
                self.context.paypal.pay_with_card(order_id, settings["merchantId"])
            dashboard.delete_account()
            self.assert_backend_error(
                410, lambda viewer=viewer, order_id=order_id: viewer.capture_tip(order_id)
            )
            if provider == "stripe":
                status = self.context.stripe.payment(order_id, settings["stripeAccountId"])["status"]
                self.assert_not_equal(status, "succeeded", "no money was taken with Stripe")
            else:
                status = self.context.paypal.order(order_id, settings["merchantId"])["status"]
                self.assert_not_equal(status, "COMPLETED", "no money was taken with PayPal")


class LifecycleNewKey(LifecycleTestCase):
    """Disconnect every device when the key is replaced, keep them listed, and let them back with the new key."""

    def run(self):
        dashboard = self.sign_in("newkeydevices")
        old = dashboard.moblin_cloud_key()
        devices = [RawDevice(self.backend.port, old, f"NK{number}") for number in [1, 2]]
        dashboard.rename_device("NK2", "Second")
        new = dashboard.new_moblin_cloud_key()["key"]
        for device in devices:
            self.assert_true(device.closed.wait(10), "the device is disconnected")
        account = dashboard.account()
        self.assert_false(account["settings"]["connected"])
        self.assert_equal(
            sorted(
                (device["id"], device["name"], device["connected"], device["reconnecting"])
                for device in account["devices"]
            ),
            [("NK1", "Moblin 1", False, False), ("NK2", "Second", False, False)],
        )
        self.assert_true(refused(self.backend.port, old))
        again = RawDevice(self.backend.port, new, "NK2")
        self.assert_true(again.ok())
        self.assert_equal(dashboard.account()["devices"][0]["name"], "Second")
        again.close()


class LifecycleStatsBounded(LifecycleTestCase):
    """Keep the operator's traffic and request statistics bounded whatever devices and clients send."""

    def run(self):
        admin = self.sign_in("admin")
        dashboard = self.sign_in("statsbounded")
        device = RawDevice(self.backend.port, dashboard.moblin_cloud_key(), "SB1")
        for number in range(150):
            device.event({f"madeUp{number}": {}})
            device.send({"response": {"id": number, "result": {f"madeUp{number}": {}}}})
        device.send({"ping": {}})
        device.wait_for(lambda message: "pong" in message)
        for number in range(150):
            requests.get(f"{self.backend.url}/made/up/{uuid.uuid4()}", timeout=10)
        stats = admin.admin_stats()
        received = stats["moblin"]["messages"]["received"]
        self.assert_true(len(received) <= 101, f"{len(received)} received kinds")
        endpoints = [endpoint["path"] for endpoint in stats["server"]["requests"]["endpoints"]]
        self.assert_false(
            any(path.startswith("/made/up/") for path in endpoints), "unknown paths are one entry"
        )
        self.assert_in("*", endpoints)
        device.close(1001)
        self.wait_until(
            lambda: dashboard.account()["devices"][0]["reconnecting"], "the device to be reconnecting"
        )
        self.assert_equal(
            admin.admin_stats()["moblin"]["connectedDevices"],
            stats["moblin"]["connectedDevices"] - 1,
            "a reconnecting device is not counted as connected",
        )


class LifecycleAssistantOfMany(LifecycleTestCase):
    """List every channel a user assists, on each platform, and only those."""

    def run(self):
        assistant = self.sign_in("manychannels")
        assistant.link(self.frontend.link_token("kick", "manychannelskick"))
        twitch = self.sign_in("channelone")
        kick = self.frontend.fake_sign_in("channeltwo", "kick")
        both = self.sign_in("channelthree")
        both.link(self.frontend.link_token("kick", "channelthreekick"))
        twitch.add_assistant("twitch", "manychannels")
        kick.add_assistant("kick", "manychannelskick")
        both.add_assistant("twitch", "manychannels")
        both.add_assistant("kick", "manychannelskick")
        assisting = assistant.account()["assisting"]
        ids = [channel["id"] for channel in assisting]
        self.assert_equal(sorted(set(ids)), sorted({twitch.page_id(), kick.page_id(), both.page_id()}))
        self.assert_equal(len(ids), len(set(ids)), f"each channel once {assisting}")
        for page in [twitch, kick, both]:
            self.assert_equal(assistant.assisted_test_tip(page.page_id()), {})
        both.remove_assistant("twitch", user_id_of("manychannels"))
        self.assert_in(both.page_id(), [channel["id"] for channel in assistant.account()["assisting"]])
        both.remove_assistant("kick", kick_user_id_of("manychannelskick"))
        self.assert_not_in(both.page_id(), [channel["id"] for channel in assistant.account()["assisting"]])


def tests(context: Context):
    return [
        LifecycleKeptOnRestart(context),
        LifecycleOneProviderLeft(context),
        LifecycleCaptureAfterDelete(context),
        LifecycleNewKey(context),
        LifecycleStatsBounded(context),
        LifecycleAssistantOfMany(context),
    ]
