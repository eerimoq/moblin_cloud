import base64
import hashlib
import hmac
import json
import time

import requests

from ..utils.backend import SESSION_SECRET
from ..utils.config import FRONTEND_URL
from ..utils.context import Context
from ..utils.devices import RecordingMoblin
from ..utils.events import EventStream
from ..utils.fake_twitch import user_id_of
from ..utils.frontend import Dashboard
from ..utils.frontend import Subscriber
from ..utils.test_case import TestCase

NOT_MODERATOR = "Only the streamer and the moderators they turned on can do this."
NO_PREVIEW = "The streamer has not let you see the stream preview."


def unpadded(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def decoded(token: str) -> bytes:
    encoded = token.split(".")[0]
    return base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4))


def signed(payload: bytes) -> str:
    tag = hmac.new(SESSION_SECRET.encode(), payload, hashlib.sha256).digest()
    return f"{unpadded(payload)}.{unpadded(tag)}"


class AccessTestCase(TestCase):
    def setup(self):
        self.skip_if_real_twitch_sign_in()


class AccessModeratorPreview(AccessTestCase):
    """Show a moderator the stream preview only once the streamer let them, and stop showing it when turned off."""

    def run(self):
        streamer = self.sign_in("previewer")
        page_id = streamer.page_id()
        moderator = self.sign_in("previewmod")
        moderator_id = user_id_of("previewmod")
        self.assert_backend_error(
            404, lambda: streamer.set_moderator_preview("twitch", moderator_id, True), "no such moderator"
        )
        streamer.add_moderator("twitch", "previewmod")
        with RecordingMoblin(self.backend.url, streamer.moblin_cloud_key(), "PV1") as device:
            device.start()
            self.wait_until(lambda: streamer.account()["devices"] != [], "the device")
            path = f"/account/pages/{page_id}/devices/PV1/preview"
            self.assert_equal(moderator.moderated_account(page_id)["preview"], False)
            self.assert_backend_error(403, lambda: EventStream(moderator, path), NO_PREVIEW)
            with EventStream(moderator, f"/account/pages/{page_id}/devices/PV1/events") as events:
                self.assert_equal(events.next()[0], "snapshot")
            account = streamer.set_moderator_preview("twitch", moderator_id, True)
            self.assert_equal([entry["preview"] for entry in account["twitchModerators"]], [True])
            self.assert_equal(moderator.moderated_account(page_id)["preview"], True)
            with EventStream(moderator, path) as frames:
                frame = frames.wait_for("frame", 10)
                assert isinstance(frame, dict)
                self.assert_true(base64.b64decode(frame["image"]).startswith(b"\x89PNG"))
            self.assert_equal(
                [
                    entry["preview"]
                    for entry in streamer.add_moderator("twitch", "PreviewMod")["twitchModerators"]
                ],
                [True],
                "adding a moderator again keeps their preview",
            )
            streamer.set_moderator_preview("twitch", moderator_id, False)
            self.assert_backend_error(403, lambda: EventStream(moderator, path), NO_PREVIEW)
            self.assert_backend_error(403, lambda: EventStream(self.sign_in("previewother"), path))
            with EventStream(streamer, path) as frames:
                frames.wait_for("frame", 10)
        streamer.remove_moderator("twitch", moderator_id)


class AccessModeratorsLimit(AccessTestCase):
    """Keep at most 50 moderators per platform, but let one listed already be added again."""

    def run(self):
        streamer = self.sign_in("manymods")
        for number in range(50):
            streamer.add_moderator("twitch", f"manymod{number}")
        self.assert_equal(len(streamer.account()["twitchModerators"]), 50)
        self.assert_backend_error(
            409, lambda: streamer.add_moderator("twitch", "manymod50"), "at most 50 moderators"
        )
        self.assert_equal(len(streamer.add_moderator("twitch", "MANYMOD7")["twitchModerators"]), 50)


class AccessDeletedModerators(AccessTestCase):
    """Remove a deleted account's moderator rows both ways, so signing in again moderates nothing and is moderated by nobody."""

    def run(self):
        streamer = self.sign_in("delstreamer")
        page_id = streamer.page_id()
        moderator = self.sign_in("delmod")
        streamer.add_moderator("twitch", "delmod")
        moderator.add_moderator("twitch", "delstreamer")
        moderator.delete_account()
        self.assert_equal(streamer.account()["twitchModerators"], [], "the deleted moderator is gone")
        self.assert_equal(streamer.account()["moderating"], [], "the deleted streamer's list is gone")
        again = self.sign_in("delmod")
        self.assert_equal(again.account()["moderating"], [])
        self.assert_backend_error(403, lambda: again.moderated_account(page_id), NOT_MODERATOR)
        streamer.add_moderator("twitch", "delmod")
        streamer.delete_account()
        self.assert_equal(again.account()["moderating"], [])
        returned = self.sign_in("delstreamer")
        self.assert_equal(returned.account()["twitchModerators"], [])
        self.assert_backend_error(403, lambda: again.moderated_account(returned.page_id()), NOT_MODERATOR)


class AccessModeratorLimits(AccessTestCase):
    """Keep a moderator to the page they moderate, its devices and what the streamer may do."""

    def run(self):
        streamer = self.sign_in("limitedstreamer")
        page_id = streamer.page_id()
        moderator = self.sign_in("limitedmod")
        streamer.add_moderator("twitch", "limitedmod")
        other = self.sign_in("limitedother")
        with RecordingMoblin(self.backend.url, other.moblin_cloud_key(), "LO1") as device:
            device.start()
            self.wait_until(lambda: other.account()["devices"] != [], "the other streamer's device")
            self.assert_backend_error(
                403, lambda: moderator.control_device(other.page_id(), "LO1", {"setMute": {"on": True}})
            )
            self.assert_refused(lambda: moderator.control_device(page_id, "LO1", {"setMute": {"on": True}}))
            self.assert_refused(
                lambda: EventStream(moderator, f"/account/pages/{page_id}/devices/LO1/events")
            )
            self.assert_equal(device.controls, [])
        self.assert_backend_error(403, lambda: moderator.moderated_settings(page_id, font="serif"))
        self.assert_equal(moderator.moderated_account(page_id)["key"], None)
        self.assert_backend_error(403, lambda: moderator.moderated_account("AAAAAAAAAAAAAAAA"), NOT_MODERATOR)
        for path in ["/account/key", "/account/paypal/connect", "/account/delete", "/account/sign-out"]:
            response = moderator.request("POST", path.replace("/account", f"/account/pages/{page_id}"))
            self.assert_true(response.status_code in [404, 405], f"{path}: {response.status_code}")
        self.assert_equal(streamer.account()["settings"]["id"], page_id)
        streamer.remove_moderator("twitch", user_id_of("limitedmod"))


class AccessTokens(AccessTestCase):
    """Accept sessions, viewer tokens and link tokens only where each belongs, and only while valid."""

    def run(self):
        dashboard = self.sign_in("tokens")
        page_id = dashboard.page_id()
        dashboard.change_settings(subEffects={"tier1": 1, "tier2": 1, "tier3": 1})
        session = dashboard.session or ""
        account, generation, issued = decoded(session).decode().rsplit(":", 2)
        self.assert_equal(
            signed(f"{account}:{generation}:{issued}".encode()), session, "tokens are signed so"
        )
        for forged in [
            signed(f"{account}:{int(generation) + 1}:{issued}".encode()),
            signed(f"{account}:{generation}:{int(time.time()) - 31 * 24 * 3600}".encode()),
            signed(f"{account}:x:{issued}".encode()),
            signed(f"{account}:{generation}".encode()),
            f"{session.split('.')[0]}.{unpadded(b'0' * 32)}",
            session + "x",
            "Bearer " + session,
        ]:
            self.assert_backend_error(401, Dashboard(self.backend.url, forged).account)
        viewer_url = self.frontend.fake_viewer_sign_in(page_id, "tokenfan1")
        viewer = viewer_url.split("#viewer=")[1]
        payload = json.loads(decoded(viewer).decode().removeprefix("viewer:"))
        self.assert_equal(signed(decoded(viewer)), viewer)
        self.assert_equal(Subscriber(self.backend.url, page_id, viewer).effect_allowance()["tier"], 1)
        old = signed(b"viewer:" + json.dumps({**payload, "issued": payload["issued"] - 25 * 3600}).encode())
        self.assert_backend_error(401, Subscriber(self.backend.url, page_id, old).effect_allowance)
        self.assert_backend_error(401, Dashboard(self.backend.url, viewer).account)
        self.assert_backend_error(401, Subscriber(self.backend.url, page_id, session).effect_allowance)
        link = self.frontend.link_token("kick", "tokenskick")
        link_payload = json.loads(decoded(link).decode().removeprefix("link:"))
        stale = signed(
            b"link:" + json.dumps({**link_payload, "issued": link_payload["issued"] - 11 * 60}).encode()
        )
        self.assert_backend_error(400, lambda: dashboard.link(stale), "Linking took too long. Try again.")
        self.assert_refused(lambda: dashboard.link(viewer))
        self.assert_backend_error(401, Subscriber(self.backend.url, page_id, link).effect_allowance)
        dashboard.change_settings(subEffects=None)


class AccessCors(AccessTestCase):
    """Let the frontend send its session and JSON bodies from the browser, and nobody else."""

    def run(self):
        response = requests.options(
            f"{self.backend.url}/account/settings",
            headers={
                "origin": FRONTEND_URL,
                "access-control-request-method": "POST",
                "access-control-request-headers": "authorization,content-type",
            },
            timeout=10,
        )
        self.assert_equal(response.headers.get("access-control-allow-origin"), FRONTEND_URL)
        allowed = response.headers.get("access-control-allow-headers", "").lower()
        self.assert_true("authorization" in allowed and "content-type" in allowed, allowed)
        self.assert_in("POST", response.headers.get("access-control-allow-methods", ""))
        response = requests.get(
            f"{self.backend.url}/account", headers={"origin": "https://evil.example"}, timeout=10
        )
        self.assert_not_equal(response.headers.get("access-control-allow-origin"), "https://evil.example")
        self.assert_not_equal(response.headers.get("access-control-allow-origin"), "*")


def tests(context: Context):
    return [
        AccessModeratorPreview(context),
        AccessModeratorsLimit(context),
        AccessDeletedModerators(context),
        AccessModeratorLimits(context),
        AccessTokens(context),
        AccessCors(context),
    ]
