import json

import requests

from ..utils.config import FRONTEND_URL
from ..utils.context import Context
from ..utils.devices import RawDevice
from ..utils.test_case import TestCase

BODIES: list[str | bytes] = [
    "",
    "not json",
    "null",
    "[]",
    "{}",
    "1e999",
    '"x"',
    json.dumps({"amount": -1, "spins": -1, "price": -1, "effect": None}),
    json.dumps({"amount": 2**64, "spins": 2**64, "price": 2**64}),
    json.dumps({"amount": "100", "spins": "1", "price": "100", "effect": 1, "message": 1, "user": []}),
    json.dumps(
        {
            "amount": 100,
            "spins": 1,
            "price": 100,
            "effect": "fart",
            "message": "\u0000\ud800",
            "user": "\u202e",
        }
    ),
    json.dumps({"amount": 100, "message": "x" * 100_000, "user": "y" * 100_000, "name": "z" * 100_000}),
    json.dumps({"name": "", "username": "", "token": "", "country": "", "on": "yes"}),
    json.dumps(
        {"currency": "€", "tips": {"amounts": [[1]]}, "spin": {"options": {}}, "effects": [], "subSpins": 1}
    ),
    b"\xff\xfe\x00garbage",
]


class RobustnessTestCase(TestCase):
    """Throw what a broken or hostile client might send at every route, which must never be a server error."""

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.dashboard = self.sign_in("robust")
        self.settings = self.context.page_with_paypal(self.dashboard)
        self.page_id = self.settings["id"]
        self.dashboard.change_settings(tips={"amounts": [100]}, effects={"price": 100})
        self.device = RawDevice(self.backend.port, self.dashboard.moblin_cloud_key(), "ROBUST")

    def teardown(self):
        if hasattr(self, "device"):
            self.device.close()
        if hasattr(self, "dashboard"):
            self.dashboard.change_settings(tips=None, effects=None)

    def post_paths(self) -> list[str]:
        page = self.page_id
        order = self.frontend.viewer(page).order_tip(100)
        return [
            f"/pages/{page}/tips/orders",
            f"/pages/{page}/tips/orders/{order}/capture",
            f"/pages/{page}/spin/orders",
            f"/pages/{page}/spin/orders/{order}/capture",
            f"/pages/{page}/effects/orders",
            f"/pages/{page}/effects/orders/{order}/capture",
            f"/pages/{page}/spin/subscriber",
            f"/pages/{page}/effects/subscriber",
            "/account/link",
            "/account/unlink/kick",
            "/account/devices/ROBUST/name",
            "/account/devices/NOPE/remove",
            "/account/settings",
            "/account/image",
            "/account/stripe/connect",
            "/account/recent/NOPE/resend",
            "/account/test/tip",
            "/account/test/spin",
            "/account/test/effect",
            "/account/assistants/twitch",
            "/account/assistants/kick",
            "/account/assistants/twitch/123/remove",
            "/account/assistants/twitch/123/preview",
            f"/account/pages/{page}/settings",
            f"/account/pages/{page}/image",
            f"/account/pages/{page}/test/tip",
            f"/account/pages/{page}/recent/NOPE/resend",
            f"/account/pages/{page}/devices/ROBUST/control",
            "/paypal/webhooks",
        ]

    def assert_no_server_error(self, response: requests.Response, what: str):
        self.assert_true(
            response.status_code < 500, f"{what}: {response.status_code} {response.text[:300]!r}"
        )


class RobustnessBodies(RobustnessTestCase):
    """Answer malformed, mistyped, huge and odd bodies with a client error, never a server error."""

    def run(self):
        for path in self.post_paths():
            for body in BODIES:
                for content_type in ["application/json", "text/plain"]:
                    response = self.dashboard.request(
                        "POST", path, data=body, headers={"content-type": content_type}
                    )
                    self.assert_no_server_error(response, f"POST {path} {content_type} {body[:60]!r}")
        self.assert_equal(self.dashboard.account()["settings"]["id"], self.page_id, "still signed in")


class RobustnessPathsAndQueries(RobustnessTestCase):
    """Answer odd ids, queries and headers with a client error, never a server error."""

    def run(self):
        for odd in ["%00", "%FF", "..", "%2e%2e%2f", "A" * 5000, "%F0%9F%92%A9", "'", "%22%3E"]:
            for path in [
                f"/pages/{odd}",
                f"/pages/{odd}/tips",
                f"/pages/{odd}/spin",
                f"/pages/{odd}/effects",
                f"/pages/twitch/{odd}",
                f"/pages/kick/{odd}",
                f"/icons/{odd}",
                f"/effects/{odd}",
                f"/account/pages/{odd}",
                f"/account/pages/{odd}/recent?kind=tip",
                f"/account/pages/{self.page_id}/devices/{odd}/events",
                f"/account/recent?kind={odd}",
                f"/account/recent?kind=tip&limit={odd}&before={odd}",
                f"/pages/addresses?logins={odd}",
                f"/auth/twitch/login?page={odd}",
                f"/auth/kick/callback?code={odd}&state={odd}",
            ]:
                response = self.dashboard.request("GET", path, stream=True)
                response.close()
                self.assert_no_server_error(response, f"GET {path[:120]}")
        for authorization in [
            "Bearer",
            "Bearer ",
            "Basic abc",
            "Bearer å",
            "Bearer " + "x" * 10000,
            "Bearer a.b.c.d",
        ]:
            response = requests.get(
                f"{self.backend.url}/account",
                headers={"authorization": authorization.encode("utf-8"), "origin": FRONTEND_URL},
                timeout=10,
            )
            self.assert_equal(response.status_code, 401, authorization[:30])


class RobustnessMoblinMessages(RobustnessTestCase):
    """Keep serving the page when its device sends odd or huge messages."""

    def run(self):
        messages: list[dict] = [
            {"identify": {}},
            {"event": {"data": {"state": {"data": []}}}},
            {"event": {"data": {"status": {"topRight": {"bitrateInfo": {"bitsPerSecond": -1}}}}}},
            {
                "event": {
                    "data": {"wheelOfLuck": {"wheel": {"options": [{"text": "x" * 100_000, "weight": -1}]}}}
                }
            },
            {
                "event": {
                    "data": {
                        "wheelOfLuck": {
                            "wheel": {"options": [{"text": str(n), "weight": 1} for n in range(10_000)]}
                        }
                    }
                }
            },
            {"preview": {"preview": "not base64"}},
            {"response": {"id": -1, "result": {}}},
            {"response": {"id": 2**70, "result": {"ok": {}}, "data": {"getSettings": {"data": 1}}}},
        ]
        for message in messages:
            self.device.send(message)
        self.device.send({"ping": {}})
        self.device.wait_for(lambda message: "pong" in message)
        viewer = self.frontend.viewer(self.page_id)
        for call in [viewer.profile, viewer.spin, viewer.tips, viewer.effects, self.dashboard.account]:
            call()
        self.assert_equal(
            self.dashboard.control_device(self.page_id, "ROBUST", {"setTorch": {"on": True}}), {}
        )


def tests(context: Context):
    return [
        RobustnessBodies(context),
        RobustnessPathsAndQueries(context),
        RobustnessMoblinMessages(context),
    ]
