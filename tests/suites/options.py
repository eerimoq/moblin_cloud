import base64

import requests

from ..utils.context import Context
from ..utils.devices import RawDevice
from ..utils.fake_kick import user_id_of as kick_user_id_of
from ..utils.fake_twitch import user_id_of
from ..utils.test_case import TestCase
from .paypal import post_webhook

PAYMENTS = [
    "--paypal-url",
    "--paypal-client-id",
    "--paypal-client-secret",
    "--paypal-partner-id",
    "--paypal-webhook-id",
    "--stripe-url",
    "--stripe-secret-key",
    "--stripe-publishable-key",
]
FAKE_SIGN_IN = ["--twitch-sign-in-url", "--kick-sign-in-url"]


class OptionsTestCase(TestCase):
    """The backend started with other options than in the other tests, and with theirs again afterwards."""

    def teardown(self):
        self.backend.restart()

    def login(self, platform: str) -> requests.Response:
        return requests.get(f"{self.backend.url}/auth/{platform}/login", allow_redirects=False, timeout=30)


class OptionsWithoutPayments(OptionsTestCase):
    """Take no payments without PayPal's and Stripe's credentials, and take them for the same streamers again with them."""

    def run(self):
        streamer = self.sign_in("optionspayments")
        merchant_id = self.context.page_with_paypal(streamer)["merchantId"]
        streamer.change_settings(tips={"amounts": [100]})
        viewer = self.frontend.viewer(streamer.page_id())
        self.backend.restart(without=PAYMENTS)
        self.assert_equal(self.frontend.config()["payments"], {"paypal": False, "stripe": False})
        streamer = self.sign_in("optionspayments")
        for call in [streamer.connect_paypal, streamer.check_paypal, streamer.connect_stripe]:
            self.assert_backend_error(503, call, "payments are unavailable")
        self.assert_is_none(viewer.tips())
        self.assert_backend_error(
            503, lambda: viewer.order_tip(100), "Tips are unavailable right now. Try again later."
        )
        self.assert_equal(
            post_webhook(self.backend.url, "MERCHANT.PARTNER-CONSENT.REVOKED", merchant_id, "good"), 503
        )
        self.backend.restart()
        self.assert_equal(self.frontend.config()["payments"], {"paypal": True, "stripe": True})
        self.assert_equal(streamer.account()["settings"]["merchantId"], merchant_id)
        self.assert_true(viewer.order_tip(100))


class OptionsWithoutSignIn(OptionsTestCase):
    """Offer no sign in without a platform's credentials, keeping who is signed in, and send streamers to Twitch and Kick themselves with them."""

    def run(self):
        streamer = self.sign_in("optionssignin")
        self.backend.restart(without=FAKE_SIGN_IN)
        self.assert_equal(self.frontend.config()["signIn"], {"twitch": False, "kick": False})
        for platform in ["twitch", "kick"]:
            self.assert_equal(self.login(platform).status_code, 503, platform)
        self.assert_equal(streamer.account()["twitch"]["login"], "optionssignin")
        self.backend.restart(
            without=FAKE_SIGN_IN,
            extra=[
                "--twitch-client-id=twitchclient",
                "--twitch-client-secret=secret",
                "--kick-client-id=kickclient",
                "--kick-client-secret=secret",
            ],
        )
        self.assert_equal(self.frontend.config()["signIn"], {"twitch": True, "kick": True})
        for platform, url, client in [
            ("twitch", "https://id.twitch.tv/", "twitchclient"),
            ("kick", "https://id.kick.com/", "kickclient"),
        ]:
            response = self.login(platform)
            location = response.headers.get("location", "")
            self.assert_equal(response.status_code, 303, platform)
            self.assert_true(location.startswith(url), location)
            self.assert_in(f"client_id={client}", location)


class OptionsWithoutSessionSecret(OptionsTestCase):
    """Sign everyone out when a backend without a session secret restarts, keeping their accounts."""

    def run(self):
        self.backend.restart(without=["--session-secret"])
        streamer = self.sign_in("optionssecret")
        page_id = streamer.page_id()
        self.backend.restart(without=["--session-secret"])
        self.assert_backend_error(401, streamer.account, "not signed in")
        self.assert_equal(self.sign_in("optionssecret").page_id(), page_id)


class OptionsAdmins(OptionsTestCase):
    """Have no admins unless told, take them as Twitch and Kick user ids, and refuse to start with one of another platform."""

    def run(self):
        twitch = self.sign_in("admin")
        kick = self.frontend.fake_sign_in("optionskickadmin", "kick")
        self.assert_backend_error(403, kick.admin_stats, "not an admin")
        self.backend.restart(without=["--admins"])
        self.assert_false(twitch.account()["admin"])
        self.assert_backend_error(403, twitch.admin_stats, "not an admin")
        admins = f"twitch:{user_id_of('admin')},kick:{kick_user_id_of('optionskickadmin')}"
        self.backend.restart(without=["--admins"], extra=["--admins", admins])
        for admin in [twitch, kick]:
            self.assert_true(admin.account()["admin"])
            self.assert_true(admin.admin_stats()["accounts"]["total"] >= 2)
        self.assert_backend_error(403, self.sign_in("optionsnotadmin").admin_stats, "not an admin")
        refused = self.backend.run(without=["--admins"], extra=["--admins", "youtube:123"])
        self.assert_not_equal(refused.returncode, 0)
        self.assert_in("expected a Twitch user ID, twitch:<ID> or kick:<ID>", refused.stderr)


class OptionsAppAttest(OptionsTestCase):
    """Let a Moblin without a valid App Attest connect while they are only counted, and refuse it once they are required."""

    def run(self):
        streamer = self.sign_in("optionsattest")
        key = streamer.moblin_cloud_key()
        nonsense = base64.b64encode(b"nonsense").decode()
        forged = {
            "keyId": nonsense,
            "attestation": nonsense,
            "attestationChallenge": nonsense,
            "assertion": nonsense,
        }
        identifies = [
            ("PLAIN", None, "no app attest in identify"),
            ("EMPTY", "nonsense", "invalid app attest: no key id"),
            ("FORGED", forged, "invalid app attest: bad attestation"),
        ]

        def connect(device_id: str, app_attest) -> RawDevice:
            identify: dict = {"streamerId": device_id, "authentication": "x"}
            if app_attest is not None:
                identify["appAttest"] = app_attest
            return RawDevice(self.backend.port, key, device_id, identify)

        self.backend.restart(without=["--allow-unattested"])
        self.assert_true(self.sign_in("admin").admin_stats()["server"]["integrations"]["appAttest"])
        for device_id, app_attest, reason in identifies:
            start = self.backend.log_position()
            device = connect(device_id, app_attest)
            self.assert_true(device.ok(), device_id)
            self.backend.wait_for_log(f"device {device_id} is not attested: {reason}", start)
            device.close()
        self.backend.restart(without=["--allow-unattested"], extra=["--require-attested-moblin"])
        for device_id, app_attest, _ in identifies:
            device = connect(device_id, app_attest)
            self.assert_false(device.ok(), device_id)
            self.assert_true(device.closed.wait(10), "the device is disconnected")
        self.assert_false(any(device["connected"] for device in streamer.account()["devices"]))


def tests(context: Context):
    return [
        OptionsWithoutPayments(context),
        OptionsWithoutSignIn(context),
        OptionsWithoutSessionSecret(context),
        OptionsAdmins(context),
        OptionsAppAttest(context),
    ]
