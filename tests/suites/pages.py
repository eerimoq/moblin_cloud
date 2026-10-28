import requests

from ..utils.config import FRONTEND_URL
from ..utils.context import Context
from ..utils.devices import RawDevice
from ..utils.test_case import TestCase


class PagesTestCase(TestCase):
    def setup(self):
        self.skip_if_real_twitch_sign_in()

    def addresses(self, logins: str) -> requests.Response:
        return requests.get(f"{self.backend.url}/pages/addresses", params={"logins": logins}, timeout=10)


class PagesAddresses(PagesTestCase):
    """Answer the Moblin website which of its streamers have a page, with its address and Moblin's icon."""

    def run(self):
        twitch = self.sign_in("addressed")
        kick = self.frontend.fake_sign_in("addressedkick", "kick")
        logins = "twitch:addressed,kick:addressedkick,twitch:addressedkick,kick:addressed,twitch:nobodyhasthis,bogus:addressed,addressed,twitch:"
        response = self.addresses(logins)
        self.assert_equal(response.status_code, 200)
        self.assert_equal(
            response.json(),
            {
                "twitch:addressed": {"url": f"{FRONTEND_URL}/twitch/addressed", "icon": None},
                "kick:addressedkick": {"url": f"{FRONTEND_URL}/kick/addressedkick", "icon": None},
            },
        )
        device = RawDevice(self.backend.port, kick.moblin_cloud_key(), "AD1")
        try:
            device.event({"icon": {"id": "AppIconKing"}})
            self.wait_until(
                lambda: self.addresses("kick:addressedkick").json()["kick:addressedkick"]["icon"] is not None,
                "the icon",
            )
        finally:
            device.close()
        icon = self.addresses("kick:addressedkick").json()["kick:addressedkick"]["icon"]
        self.assert_true(icon.endswith("/icons/king"), icon)
        served = requests.get(icon, timeout=10)
        self.assert_equal([served.status_code, served.headers["content-type"]], [200, "image/png"])
        twitch.delete_account()
        self.assert_equal(self.addresses("twitch:addressed").json(), {})


class PagesAddressesLimit(PagesTestCase):
    """Look at most the first 100 logins up at once."""

    def run(self):
        self.sign_in("addresslimit")
        logins = [f"twitch:filler{number}" for number in range(99)] + ["twitch:addresslimit"]
        response = self.addresses(",".join(logins))
        self.assert_equal(response.status_code, 200)
        self.assert_equal(list(response.json()), ["twitch:addresslimit"])
        response = self.addresses(",".join(["twitch:onemore"] + logins))
        self.assert_true(response.status_code < 500, response.status_code)
        if response.ok:
            self.assert_equal(response.json(), {}, "the 101st login is not looked up")
        for query in ["", "?logins=", "?other=1"]:
            response = requests.get(f"{self.backend.url}/pages/addresses{query}", timeout=10)
            self.assert_true(response.status_code < 500, f"{query}: {response.status_code}")


class PagesPublicRoutes(PagesTestCase):
    """Serve icons and answer unknown pages, logins and icons with not found."""

    def run(self):
        for icon in ["plain", "king", "san-diego"]:
            response = requests.get(f"{self.backend.url}/icons/{icon}", timeout=10)
            self.assert_equal(
                [response.status_code, response.headers["content-type"]], [200, "image/png"], icon
            )
        for path in [
            "/icons/nope",
            "/icons/AppIconKing",
            "/pages/twitch/nobody_has_this",
            "/pages/kick/nobody_has_this",
            "/pages/youtube/someone",
            "/pages/AAAAAAAAAAAAAAAA",
            "/pages/AAAAAAAAAAAAAAAA/image",
            "/pages/AAAAAAAAAAAAAAAA/events",
            "/effects/nope",
        ]:
            response = requests.get(f"{self.backend.url}{path}", timeout=10)
            self.assert_equal(response.status_code, 404, path)
        for path in [
            "/pages/AAAAAAAAAAAAAAAA/spin/subscriber",
            "/pages/AAAAAAAAAAAAAAAA/effects/subscriber",
            "/pages/AAAAAAAAAAAAAAAA/effects",
            "/pages/AAAAAAAAAAAAAAAA/tips",
            "/pages/%00/tips",
            f"/pages/{'A' * 10000}",
            "/pages/twitch/%F0%9F%92%A9",
        ]:
            response = requests.get(f"{self.backend.url}{path}", timeout=10)
            self.assert_true(response.status_code < 500, f"{path}: {response.status_code}")


def tests(context: Context):
    return [
        PagesAddresses(context),
        PagesAddressesLimit(context),
        PagesPublicRoutes(context),
    ]
