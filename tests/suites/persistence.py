from ..utils.context import Context
from ..utils.test_case import TestCase


class PersistenceBackendRestart(TestCase):
    """Keep accounts, sessions, pages and their settings when the backend restarts, and let Moblin reconnect."""

    def run(self):
        dashboard = self.sign_in()
        dashboard.save_settings(message="Still here", accent="blue")
        account = dashboard.account()
        if self.context.moblin is not None:
            self.context.moblin.connect_to_backend(account["key"])
        self.backend.restart()
        restarted = dashboard.account()
        self.assert_equal(restarted["key"], account["key"])
        self.assert_equal(restarted["settings"]["id"], account["settings"]["id"])
        self.assert_equal(restarted["settings"]["merchantId"], account["settings"]["merchantId"])
        profile = self.website.viewer(account["settings"]["id"]).profile()
        self.assert_equal([profile["message"], profile["accent"]], ["Still here", "blue"])
        self.assert_equal(
            self.website.id_of_twitch_login(account["twitch"]["login"]), account["settings"]["id"]
        )
        if self.context.moblin is not None:
            self.wait_until(lambda: dashboard.account()["settings"]["connected"], "Moblin to reconnect")
        dashboard.save_settings()


def tests(context: Context):
    return [PersistenceBackendRestart(context)]
