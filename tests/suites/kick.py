from urllib.parse import parse_qs
from urllib.parse import urlsplit

from ..utils.config import FRONTEND_URL
from ..utils.context import Context
from ..utils.devices import RecordingMoblin
from ..utils.fake_kick import user_id_of as kick_user_id_of
from ..utils.frontend import Dashboard
from ..utils.frontend import Subscriber
from ..utils.frontend import viewer_of
from ..utils.test_case import TestCase

TAKEN = "This Kick account already has its own page. Delete it first."


class KickTestCase(TestCase):
    """Streamers signing in with the fake Kick, and Twitch streamers linking a Kick account to their page."""

    def setup(self):
        self.skip_if_real_twitch_sign_in()

    def kick_sign_in(self, login: str) -> Dashboard:
        return self.frontend.fake_sign_in(login, "kick")

    def page_of(self, platform: str, login: str) -> str:
        return self.frontend.get(f"/pages/{platform}/{login}")["id"]


class KickSignIn(KickTestCase):
    """Sign in with Kick, which gives the streamer a page under their Kick login only."""

    def run(self):
        dashboard = self.kick_sign_in("kicker")
        account = dashboard.account()
        self.assert_equal(
            [account["name"], account["twitch"], account["kick"]],
            ["kicker", None, {"login": "kicker", "name": "kicker", "avatar": None}],
        )
        page_id = account["settings"]["id"]
        self.assert_equal(self.page_of("kick", "kicker"), page_id)
        self.assert_equal(self.page_of("kick", "KICKER"), page_id)
        self.assert_backend_error(404, lambda: self.page_of("twitch", "kicker"))
        profile = self.frontend.viewer(page_id).profile()
        self.assert_equal([profile["twitch"], profile["kick"]], [None, account["kick"]])
        self.assert_equal(self.kick_sign_in("kicker").page_id(), page_id)
        twitch = self.sign_in("kicker")
        self.assert_not_equal(twitch.page_id(), page_id, "the same login on Twitch is another user")
        self.assert_equal(twitch.account()["kick"], None)


class KickLink(KickTestCase):
    """Link a Kick account to a Twitch streamer's page, which gives the page both addresses."""

    def run(self):
        dashboard = self.sign_in("linker")
        page_id = dashboard.page_id()
        account = dashboard.link(self.frontend.link_token("kick", "linkerkick"))
        self.assert_equal(account["kick"], {"login": "linkerkick", "name": "linkerkick", "avatar": None})
        self.assert_equal(account["name"], "linker")
        self.assert_equal(dashboard.account(), account)
        self.assert_equal(self.page_of("kick", "linkerkick"), page_id)
        self.assert_equal(self.page_of("twitch", "linker"), page_id)
        profile = self.frontend.viewer(page_id).profile()
        self.assert_equal([profile["twitch"], profile["kick"]], [account["twitch"], account["kick"]])
        kick = self.kick_sign_in("linkerkick")
        self.assert_equal(kick.page_id(), page_id, "signing in with the linked Kick opens the same page")
        self.assert_equal(kick.moblin_cloud_key(), dashboard.moblin_cloud_key())
        self.assert_equal(
            dashboard.link(self.frontend.link_token("kick", "linkerkick"))["kick"], account["kick"]
        )
        twitch = dashboard.link(self.frontend.link_token("twitch", "linker"))
        self.assert_equal(twitch["twitch"], account["twitch"])


class KickLinkRefused(KickTestCase):
    """Refuse linking a Kick account that has its own page, broken or reused link tokens and tokens as sessions."""

    def run(self):
        owner = self.kick_sign_in("takenkick")
        taker = self.sign_in("taker")
        token = self.frontend.link_token("kick", "takenkick")
        self.assert_backend_error(409, lambda: taker.link(token), TAKEN)
        self.assert_equal(taker.account()["kick"], None)
        self.assert_equal(self.page_of("kick", "takenkick"), owner.page_id())
        for broken in ["", "garbage", token[:-2], taker.session or ""]:
            self.assert_refused(lambda broken=broken: taker.link(broken))
        self.assert_signed_out(Dashboard(self.backend.url, token))
        self.assert_backend_error(401, lambda: Dashboard(self.backend.url, None).link(token))
        owner.delete_account()
        self.assert_equal(taker.link(token)["kick"]["login"], "takenkick")
        other = self.sign_in("othertaker")
        self.assert_backend_error(409, lambda: other.link(token), TAKEN)
        self.assert_equal(self.page_of("kick", "takenkick"), taker.page_id())

    def assert_signed_out(self, dashboard: Dashboard):
        self.assert_backend_error(401, dashboard.account, "not signed in")


class KickLinkCancelled(KickTestCase):
    """Return to the Account page with linkFailed when the streamer cancels on Kick."""

    def run(self):
        dashboard = self.sign_in("linkcancel")
        callback_url, browser = self.frontend._start_sign_in(  # pylint: disable=protected-access
            {"link": "1"}, "linkcancelkick", "kick"
        )
        state = parse_qs(urlsplit(callback_url).query)["state"][0]
        returned = self.frontend.finish_sign_in(
            f"{self.backend.url}/auth/kick/callback?error=access_denied&state={state}", browser
        )
        self.assert_equal(returned, f"{FRONTEND_URL}/account/#linkFailed")
        self.assert_equal(dashboard.account()["kick"], None)
        self.assert_backend_error(404, lambda: self.page_of("kick", "linkcancelkick"))


class KickUnlink(KickTestCase):
    """Unlink either platform but the last one, which takes the page's address on it away."""

    def run(self):
        dashboard = self.sign_in("unlinker")
        page_id = dashboard.page_id()
        key = dashboard.moblin_cloud_key()
        self.assert_backend_error(
            409,
            lambda: dashboard.unlink("twitch"),
            "You can't unlink the only way to sign in. Link another platform first.",
        )
        self.assert_refused(lambda: dashboard.unlink("kick"))
        self.assert_refused(lambda: dashboard.unlink("youtube"))
        dashboard.link(self.frontend.link_token("kick", "unlinkerkick"))
        account = dashboard.unlink("twitch")
        self.assert_equal([account["twitch"], account["name"]], [None, "unlinkerkick"])
        self.assert_equal([account["settings"]["id"], account["key"]], [page_id, key])
        self.assert_backend_error(404, lambda: self.page_of("twitch", "unlinker"))
        self.assert_equal(self.page_of("kick", "unlinkerkick"), page_id)
        self.assert_equal(self.frontend.viewer(page_id).profile()["twitch"], None)
        self.assert_backend_error(409, lambda: dashboard.unlink("kick"))
        twitch = self.sign_in("unlinker")
        self.assert_not_equal(twitch.page_id(), page_id, "the unlinked Twitch account gets a page of its own")
        self.assert_equal(self.kick_sign_in("unlinkerkick").page_id(), page_id)


class KickUnlinkSignsViewersOut(KickTestCase):
    """Refuse viewer sign ins of a platform the page no longer has linked."""

    def run(self):
        dashboard = self.sign_in("unlinkviewers")
        page_id = dashboard.page_id()
        dashboard.link(self.frontend.link_token("kick", "unlinkviewerskick"))
        dashboard.change_settings(subEffects={"tier1": 1, "tier2": 1, "tier3": 1})
        twitch = self.frontend.subscriber(page_id, "unlinkfan1")
        kick = self.frontend.subscriber(page_id, "unlinkfan1", "kick")
        self.assert_equal(twitch.effect_allowance()["platform"], "twitch")
        self.assert_equal(kick.effect_allowance()["platform"], "kick")
        dashboard.unlink("kick")
        self.assert_backend_error(401, kick.effect_allowance)
        self.assert_backend_error(401, lambda: kick.play_effect("fart"))
        self.assert_equal(twitch.effect_allowance()["left"], 1)
        self.assert_backend_error(
            404, lambda: self.frontend.start_viewer_sign_in(page_id, "unlinkfan2", "kick")
        )
        dashboard.change_settings(subEffects=None)


class KickSubscribers(KickTestCase):
    """Let Kick subscribers of a Kick streamer spin the wheel and play effects at tier 1."""

    def run(self):
        dashboard = self.kick_sign_in("kickstreamer")
        page_id = dashboard.page_id()
        dashboard.change_settings(
            subSpins={"tier1": 1, "tier2": 0, "tier3": 0}, subEffects={"tier1": 1, "tier2": 0, "tier3": 0}
        )
        self.assert_backend_error(
            404, lambda: self.frontend.start_viewer_sign_in(page_id, "kickfan1", "twitch")
        )
        page_url = self.frontend.fake_viewer_sign_in(page_id, "kickfan3", "kick")
        self.assert_true(page_url.startswith(f"{FRONTEND_URL}/kick/kickstreamer#viewer="), page_url)
        fan = Subscriber(self.frontend.url, page_id, viewer_of(page_url) or "")
        self.assert_equal(
            fan.spins(), {"platform": "kick", "name": "kickfan3", "tier": 1, "allowance": 1, "left": 1}
        )
        self.assert_equal(self.frontend.subscriber(page_id, "kickfanx", "kick").spins()["tier"], None)
        self.assert_backend_error(
            403, lambda: self.frontend.subscriber(page_id, "kickfany", "kick").play_effect("fart")
        )
        with RecordingMoblin(self.backend.url, dashboard.moblin_cloud_key(), "KICK", ["Dance"]) as device:
            device.start()
            viewer = self.frontend.viewer(page_id)
            self.wait_until(lambda: viewer.spin()["wheel"] is not None, "the wheel to be on the page")
            self.assert_equal(fan.spin_wheel("Go"), {"left": 0})
            self.assert_equal(fan.play_effect("fart"), {"left": 0})
            self.assert_backend_error(409, fan.spin_wheel)
            self.wait_until(lambda: len(device.tips) == 2, "the spin and the effect")
            self.assert_equal(sorted(tip["name"] for tip in device.tips), ["kickfan3", "kickfan3"])
        self.assert_equal([item["user"] for item in dashboard.recent("spin")["items"]], ["kickfan3"])
        dashboard.change_settings(subSpins=None, subEffects=None)


class KickAllowancesApart(KickTestCase):
    """Count a Twitch and a Kick viewer of the same login apart on a page with both linked."""

    def run(self):
        dashboard = self.sign_in("bothplatforms")
        page_id = dashboard.page_id()
        dashboard.link(self.frontend.link_token("kick", "bothplatformskick"))
        dashboard.change_settings(subEffects={"tier1": 1, "tier2": 1, "tier3": 1})
        twitch = self.frontend.subscriber(page_id, "samefan1")
        kick = self.frontend.subscriber(page_id, "samefan1", "kick")
        self.assert_equal(twitch.play_effect("fart"), {"left": 0})
        self.assert_equal(kick.effect_allowance()["left"], 1)
        self.assert_equal(kick.play_effect("fart"), {"left": 0})
        page_url = self.frontend.fake_viewer_sign_in(page_id, "samefan1", "kick")
        self.assert_true(page_url.startswith(f"{FRONTEND_URL}/kick/bothplatformskick#viewer="), page_url)
        dashboard.change_settings(subEffects=None)


class KickModerators(KickTestCase):
    """Let a Kick user the streamer added as a Kick moderator help, also after unlinking and linking Kick again."""

    def run(self):
        dashboard = self.sign_in("kickmodded")
        page_id = dashboard.page_id()
        self.assert_backend_error(409, lambda: dashboard.add_moderator("kick", "kmod"), "Link Kick first.")
        dashboard.link(self.frontend.link_token("kick", "kickmoddedkick"))
        self.assert_backend_error(
            409, lambda: dashboard.add_moderator("kick", "kickmoddedkick"), "That is your own Kick account."
        )
        self.assert_backend_error(
            404, lambda: dashboard.add_moderator("kick", "nobody"), "There is no Kick user named nobody."
        )
        self.assert_backend_error(404, lambda: dashboard.add_moderator("youtube", "kmod"), "no such platform")
        account = dashboard.add_moderator("kick", "KMod")
        self.assert_equal(
            account["kickModerators"],
            [
                {
                    "id": kick_user_id_of("kmod"),
                    "login": "kmod",
                    "name": "kmod",
                    "preview": False,
                    "goLive": False,
                    "editTips": False,
                }
            ],
        )
        self.assert_equal(account["twitchModerators"], [])
        moderator = self.kick_sign_in("kmod")
        moderating = moderator.account()["moderating"]
        self.assert_equal(
            [(channel["id"], channel["platform"]) for channel in moderating], [(page_id, "kick")]
        )
        self.assert_equal(moderating[0]["login"], "kickmoddedkick")
        self.assert_equal(moderator.moderated_test_tip(page_id), {})
        dashboard.unlink("kick")
        self.assert_equal(moderator.account()["moderating"], [])
        self.assert_backend_error(403, lambda: moderator.moderated_test_tip(page_id))
        dashboard.link(self.frontend.link_token("kick", "kickmoddedkick"))
        self.assert_equal([channel["id"] for channel in moderator.account()["moderating"]], [page_id])
        dashboard.remove_moderator("kick", kick_user_id_of("kmod"))
        self.assert_equal(moderator.account()["moderating"], [])
        self.assert_backend_error(
            404, lambda: dashboard.remove_moderator("kick", kick_user_id_of("kmod")), "no such moderator"
        )


def tests(context: Context):
    return [
        KickSignIn(context),
        KickLink(context),
        KickLinkRefused(context),
        KickLinkCancelled(context),
        KickUnlink(context),
        KickUnlinkSignsViewersOut(context),
        KickSubscribers(context),
        KickAllowancesApart(context),
        KickModerators(context),
    ]
