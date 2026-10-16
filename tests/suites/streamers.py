import base64
import secrets

import systest

from ..utils.context import Context
from ..utils.test_case import TestCase
from ..utils.utils import manual_confirmation


def channels(*pairs: tuple[str, str]) -> list[dict]:
    return [{"platform": platform, "name": name} for platform, name in pairs]


def unique_name(prefix: str) -> str:
    return f"{prefix}_{secrets.token_hex(4)}"


class StreamersTestCase(TestCase):
    def went_live(self, *channel: tuple[str, str]):
        response = self.website.went_live({"channels": channels(*channel)})
        self.assert_equal(response.status_code, 204, response.text)

    def listed(self, name: str) -> dict | None:
        for streamer in self.website.streamers():
            for channel in streamer["channels"]:
                if channel["name"].lower() == name.lower():
                    return streamer
        return None

    def skip_if_attested(self):
        if not self.context.config.allow_unattested():
            raise systest.TestCaseSkippedError("The backend only accepts attested live posts.")


class Challenge(StreamersTestCase):
    """Get one-time challenges for live posts."""

    def run(self):
        first = base64.b64decode(self.website.challenge())
        second = base64.b64decode(self.website.challenge())
        self.assert_equal(len(first), 32)
        self.assert_not_equal(first, second)


class UnattestedLivePostRefused(StreamersTestCase):
    """Refuse live posts that are not signed by the Moblin app with App Attest."""

    def run(self):
        if self.context.config.allow_unattested():
            raise systest.TestCaseSkippedError("The backend accepts unattested live posts.")
        name = unique_name("unattested")
        body = {
            "channels": channels(("twitch", name)),
            "challenge": self.website.challenge(),
            "keyId": base64.b64encode(secrets.token_bytes(32)).decode(),
            "attestation": base64.b64encode(b"not an attestation").decode(),
            "attestationChallenge": base64.b64encode(secrets.token_bytes(32)).decode(),
        }
        for headers in [None, {"moblin-assertion": base64.b64encode(b"not an assertion").decode()}]:
            response = self.website.went_live(body, headers)
            self.assert_equal(response.status_code, 401, response.text)
        response = self.website.went_live({"channels": channels(("twitch", name))})
        self.assert_equal(response.status_code, 401, response.text)
        self.assert_is_none(self.listed(name))


class StreamerGoesLive(StreamersTestCase):
    """List a streamer who went live on several platforms as one streamer."""

    def run(self):
        self.skip_if_attested()
        name = unique_name("live")
        self.went_live(("twitch", name), ("kick", name), ("youtube", name))
        streamer = self.listed(name)
        self.assert_is_not_none(streamer)
        assert streamer is not None
        self.assert_equal(
            [(channel["platform"], channel["name"]) for channel in streamer["channels"]],
            [("twitch", name), ("kick", name), ("youtube", name)],
        )
        for channel in streamer["channels"]:
            self.assert_in("avatar", channel)
            self.assert_in("displayName", channel)
            self.assert_in("live", channel)


class SameStreamerGoesLiveAgain(StreamersTestCase):
    """Move a streamer who goes live again to the top, matching channels case-insensitively."""

    def run(self):
        self.skip_if_attested()
        first = unique_name("first")
        second = unique_name("second")
        self.went_live(("twitch", first))
        self.went_live(("twitch", second))
        self.went_live(("twitch", first.upper()), ("kick", first))
        names = [streamer["channels"][0]["name"].lower() for streamer in self.website.streamers()]
        self.assert_less(names.index(first), names.index(second))
        self.assert_equal(names.count(first), 1)


class StreamersAreCapped(StreamersTestCase):
    """List at most six streamers, newest first."""

    def run(self):
        self.skip_if_attested()
        names = [unique_name(f"capped{number}") for number in range(8)]
        for name in names:
            self.went_live(("kick", name))
        listed = [streamer["channels"][0]["name"] for streamer in self.website.streamers()]
        self.assert_equal(len(listed), 6)
        for name in names[:2]:
            self.assert_not_in(name, listed)


class InvalidLivePost(StreamersTestCase):
    """Refuse live posts with no, too many or badly named channels."""

    def run(self):
        self.skip_if_attested()
        bodies: list[dict] = [
            {"channels": []},
            {"channels": channels(*[("twitch", f"too_many_{number}") for number in range(6)])},
            {"channels": channels(("twitch", "bad name"))},
            {"channels": channels(("twitch", 41 * "a"))},
            {"channels": channels(("twitch", ""))},
            {"channels": channels(("facebook", "someone"))},
            {"channels": "twitch"},
        ]
        for body in bodies:
            response = self.website.went_live(body)
            self.assert_equal(response.status_code, 400, f"{body}: {response.text}")


class TwitchProfileLookedUp(StreamersTestCase):
    """Look up the display name and avatar of a listed Twitch streamer and whether they are live."""

    def run(self):
        self.skip_if_attested()
        self.skip_if_no_twitch()
        self.went_live(("twitch", "eerimoq"))

        def looked_up() -> bool:
            streamer = self.listed("eerimoq")
            return streamer is not None and streamer["channels"][0]["displayName"] is not None

        self.wait_until(looked_up, "the Twitch profile to be looked up")
        streamer = self.listed("eerimoq")
        assert streamer is not None
        channel = streamer["channels"][0]
        self.assert_equal(channel["displayName"].lower(), "eerimoq")
        self.assert_true(channel["avatar"].startswith("https://"))
        self.assert_in(channel["live"], [True, False])


class KickProfileLookedUp(StreamersTestCase):
    """Look up the display name and avatar of a listed Kick streamer."""

    def run(self):
        self.skip_if_attested()
        self.went_live(("twitch", "eerimoq"), ("kick", "eerimoq"))

        def looked_up() -> bool:
            streamer = self.listed("eerimoq")
            return streamer is not None and any(
                channel["platform"] == "kick" and channel["displayName"] is not None
                for channel in streamer["channels"]
            )

        self.wait_until(looked_up, "the Kick profile to be looked up", timeout=120)


class MoblinGoesLive(StreamersTestCase):
    """List the streamer when Moblin goes live with listing on moblin.app turned on, attested with App Attest."""

    def run(self):
        self.skip_if_no_moblin()
        assert self.context.moblin is not None
        if self.context.moblin.is_fake():
            raise systest.TestCaseSkippedError("Needs a real Moblin device.")
        self.skip_if_not_interactive()
        if self.context.config.allow_unattested():
            raise systest.TestCaseSkippedError("The backend accepts unattested live posts.")
        position = self.backend.log_position()
        manual_confirmation(
            "In a Moblin debug build with MOBLIN_WEBSITE_API_URL set to "
            f"http://{self.context.config.tester_ip_address()}:{self.backend.port}, go live with a stream "
            "logged in to Twitch, Kick or YouTube and listing on moblin.app turned on."
        )
        mo = self.backend.wait_for_log(r"\] \w+/([\w.-]+)[, ].*went live", start=position, timeout=120)
        self.assert_false(self.backend.has_logged("rejected live post", start=position))
        self.assert_is_not_none(self.listed(mo.group(1)))


class TwitchLive(StreamersTestCase):
    """Tell whether Erik is live on Twitch, or that it can't be known without Twitch credentials."""

    def run(self):
        if self.context.config.twitch() is None:
            self.assert_backend_error(503, self.website.twitch_live, "no Twitch client ID and secret")
            return
        live = self.website.twitch_live()
        self.assert_in(live["live"], [True, False])
        self.assert_not_equal(live["channel"], "")


def tests(context: Context):
    return [
        Challenge(context),
        UnattestedLivePostRefused(context),
        StreamerGoesLive(context),
        SameStreamerGoesLiveAgain(context),
        StreamersAreCapped(context),
        InvalidLivePost(context),
        TwitchProfileLookedUp(context),
        KickProfileLookedUp(context),
        MoblinGoesLive(context),
        TwitchLive(context),
    ]
