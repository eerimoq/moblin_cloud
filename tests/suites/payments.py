import logging
import time
from collections.abc import Callable

from ..utils.config import Capability
from ..utils.context import Context
from ..utils.moblin import Moblin
from ..utils.moblin import wheel_option
from ..utils.test_case import TestCase
from ..utils.website import BackendError

LOGGER = logging.getLogger(__name__)
SONG = "https://music.apple.com/us/album/take-on-me/380907762?i=380907765"
SONG_LINK = "https://music.apple.com/song/380907765?i=380907765"
OTHER_SONG = "https://music.apple.com/us/album/billie-jean/269572838?i=269573364"
MISSING_SONG = "https://music.apple.com/us/song/does-not-exist/99999999999"
TIPS = {"amounts": [100, 700]}
SPIN = {"options": [{"spins": 1, "price": 100}, {"spins": 3, "price": 250}]}
WHEEL = [wheel_option("Push-ups"), wheel_option("Sing a song", 2), wheel_option("Nothing", 3)]


class PaymentTestCase(TestCase):
    """A viewer paying on the page of the signed in streamer, whose PayPal sandbox seller is connected."""

    def setup(self):
        self.skip_if_no_paypal()
        self.dashboard = self.sign_in()
        self.settings = self.context.page_with_paypal(self.dashboard)
        self.merchant_id = self.settings["merchantId"]
        self.viewer = self.website.viewer(self.settings["id"])
        self.dashboard.save_settings(currency="USD")

    def teardown(self):
        if hasattr(self, "dashboard"):
            self.dashboard.save_settings(currency="USD")

    @property
    def moblin(self) -> Moblin:
        assert self.context.moblin is not None
        return self.context.moblin

    def connect_moblin(self, wheel: list[dict] | None = None):
        self.moblin.connect_to_backend(self.dashboard.secret_key(), wheel)
        self.wait_until(lambda: self.dashboard.account()["settings"]["connected"], "Moblin to be connected")

    def pay(self, order_id: str):
        """Pays the order with a sandbox card, like the viewer in the popup of PayPal's button."""
        assert self.context.paypal_sandbox is not None
        self.context.paypal_sandbox.pay_with_card(order_id, self.merchant_id)

    def capture(self, capture: Callable[[str], dict], order_id: str) -> dict:
        """Captures like the page, trying again when the PayPal sandbox is temporarily unavailable."""
        for attempt in range(3):
            try:
                return capture(order_id)
            except BackendError as error:
                if error.status != 502 or attempt == 2:
                    raise
                LOGGER.debug("Capturing %s failed with %s, trying again", order_id, error)
                time.sleep(5)
        raise AssertionError("unreachable")

    def assert_paid(self, order_id: str, value: str, currency: str = "USD"):
        assert self.context.paypal_sandbox is not None
        order = self.context.paypal_sandbox.order(order_id, self.merchant_id)
        self.assert_equal(order["status"], "COMPLETED")
        unit = order["purchase_units"][0]
        self.assert_equal(unit["payee"]["merchant_id"], self.merchant_id)
        self.assert_equal([unit["amount"]["currency_code"], unit["amount"]["value"]], [currency, value])
        capture = unit["payments"]["captures"][0]
        self.assert_equal(capture["status"], "COMPLETED")

    def assert_not_paid(self, order_id: str):
        assert self.context.paypal_sandbox is not None
        order = self.context.paypal_sandbox.order(order_id, self.merchant_id)
        self.assert_not_equal(order["status"], "COMPLETED")


class TipNotApproved(PaymentTestCase):
    """Refuse to capture a tip the viewer never paid, and captures of the order as something else."""

    def run(self):
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.viewer.order_tip(700, "Viewer", "Hi!")
        self.assert_backend_error(
            402, lambda: self.capture(self.viewer.capture_tip, order_id), "The payment was not completed."
        )
        gone = "This payment is unknown or expired, so no money was taken."
        self.assert_backend_error(410, lambda: self.capture(self.viewer.capture_song, order_id), gone)
        self.assert_backend_error(410, lambda: self.capture(self.viewer.capture_spin, order_id), gone)
        other = self.website.viewer("AAAAAAAAAAAAAAAA")
        self.assert_backend_error(410, lambda: other.capture_tip(order_id), gone)
        self.assert_backend_error(410, lambda: self.viewer.capture_tip("UNKNOWN123"), gone)
        self.assert_not_paid(order_id)


class InvalidTip(PaymentTestCase):
    """Refuse tips out of range, with too long messages, or when tips are off."""

    def run(self):
        self.assert_backend_error(
            400, lambda: self.viewer.order_tip(100), "The streamer no longer takes tips. Reload the page."
        )
        self.dashboard.save_settings(tips=TIPS)
        self.assert_backend_error(400, lambda: self.viewer.order_tip(49), "Tip from 0.50 to 1000.")
        self.assert_backend_error(400, lambda: self.viewer.order_tip(100_001), "Tip from 0.50 to 1000.")
        self.assert_backend_error(
            400,
            lambda: self.viewer.order_tip(100, message=201 * "x"),
            "The message can be at most 200 characters.",
        )
        self.viewer.order_tip(333, message=200 * "x")


class TipPaid(PaymentTestCase):
    """Tip with PayPal, which pays the streamer's PayPal account and shows the tip in Moblin."""

    def run(self):
        if self.context.moblin is not None:
            self.connect_moblin()
        self.dashboard.save_settings(currency="EUR", tips=TIPS)
        tips = self.viewer.tips()
        assert tips is not None
        self.assert_equal(tips["currency"], "EUR")
        order_id = self.viewer.order_tip(700, "  Viewer  ", "  Great stream!  ")
        self.pay(order_id)
        self.assert_equal(self.capture(self.viewer.capture_tip, order_id), {})
        self.assert_paid(order_id, "7.00", "EUR")
        self.assert_backend_error(
            410,
            lambda: self.capture(self.viewer.capture_tip, order_id),
            "This payment is unknown or expired, so no money was taken.",
        )
        if self.context.moblin is None:
            return
        tip = self.moblin.wait_for_backend_request("tip", order_id)
        self.assert_equal(
            tip.body,
            {
                "name": "Viewer",
                "message": "Great stream!",
                "amount": "7.00",
                "currency": "EUR",
                "requestId": order_id,
            },
        )


class AnonymousTipPaid(PaymentTestCase):
    """Tip without a name or message."""

    def run(self):
        self.skip_if_no_moblin()
        self.connect_moblin()
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.viewer.order_tip(100)
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        tip = self.moblin.wait_for_backend_request("tip", order_id)
        self.assert_equal([tip.body["name"], tip.body["message"]], ["Someone", ""])


class TipDeliveredWhenMoblinConnects(PaymentTestCase):
    """Take a tip while Moblin is offline and show it in Moblin when it connects."""

    def run(self):
        self.skip_if_no_moblin()
        self.moblin.disconnect_from_backend()
        self.dashboard.save_settings(tips=TIPS)
        self.assert_false(self.dashboard.account()["settings"]["connected"])
        order_id = self.viewer.order_tip(100, "Early")
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        self.assert_paid(order_id, "1.00")
        self.connect_moblin()
        tip = self.moblin.wait_for_backend_request("tip", order_id)
        self.assert_equal(tip.body["name"], "Early")


class TipDeliveredAfterBackendRestart(PaymentTestCase):
    """Keep a tip that Moblin has not got yet when the backend restarts, and deliver it afterwards."""

    def run(self):
        self.skip_if_no_moblin()
        self.moblin.disconnect_from_backend()
        self.dashboard.save_settings(tips=TIPS)
        order_id = self.viewer.order_tip(700, "Patient")
        self.pay(order_id)
        self.capture(self.viewer.capture_tip, order_id)
        self.backend.restart()
        self.connect_moblin()
        self.moblin.wait_for_backend_request("tip", order_id)


class SpinNeedsWheel(PaymentTestCase):
    """Refuse wheel spins when the wheel is off in the dashboard or Moblin has no wheel."""

    def run(self):
        self.skip_if_no_moblin()
        self.connect_moblin()
        self.assert_backend_error(
            400, lambda: self.viewer.order_spin(1), "The streamer turned the wheel off. Reload the page."
        )
        self.dashboard.save_settings(spin=SPIN)
        self.assert_equal(self.viewer.spin()["wheel"], None)
        self.assert_backend_error(
            404,
            lambda: self.viewer.order_spin(1),
            "The streamer's wheel is not available right now. Try again when they are live.",
        )


class SpinPaid(PaymentTestCase):
    """Buy wheel spins with PayPal, which spins the wheel in Moblin."""

    def run(self):
        self.skip_if_no_moblin()
        self.connect_moblin(WHEEL)
        self.dashboard.save_settings(spin=SPIN)
        self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "the wheel to be on the page")
        spin = self.viewer.spin()
        self.assert_equal(
            spin["wheel"],
            {"options": [{"text": option["text"], "weight": option["weight"]} for option in WHEEL]},
        )
        self.assert_equal(spin["spin"]["options"], SPIN["options"])
        self.assert_backend_error(
            400, lambda: self.viewer.order_spin(2), "The streamer changed the prices. Reload the page."
        )
        order_id = self.viewer.order_spin(3, "Spinner", "Spin it!")
        self.pay(order_id)
        self.assert_equal(self.capture(self.viewer.capture_spin, order_id), {})
        self.assert_paid(order_id, "2.50")
        request = self.moblin.wait_for_backend_request("spinWheelOfLuck", order_id)
        self.assert_equal(
            request.body,
            {"name": "Spinner", "message": "Spin it!", "count": 3, "requestId": order_id},
        )


class SpinWhenWheelIsGone(PaymentTestCase):
    """Do not take the money for spins when Moblin's wheel is gone before the capture."""

    def run(self):
        self.skip_if_no_moblin()
        self.connect_moblin(WHEEL)
        self.dashboard.save_settings(spin=SPIN)
        self.wait_until(lambda: self.viewer.spin()["wheel"] is not None, "the wheel to be on the page")
        order_id = self.viewer.order_spin(1)
        self.pay(order_id)
        self.connect_moblin()
        self.wait_until(lambda: self.viewer.spin()["wheel"] is None, "the wheel to be gone")
        self.assert_backend_error(
            404,
            lambda: self.capture(self.viewer.capture_spin, order_id),
            "The streamer's wheel is gone, so your payment was not taken.",
        )
        self.assert_not_paid(order_id)


class SongRequestTestCase(PaymentTestCase):
    def setup(self):
        super().setup()
        self.dashboard.save_settings(paywall={"price": 150})


class InvalidSongRequest(SongRequestTestCase):
    """Refuse song requests that are not Apple Music song links, or when the streamer is offline."""

    def run(self):
        if self.context.moblin is not None:
            self.moblin.disconnect_from_backend()
        for song in [
            "Take on me",
            "https://open.spotify.com/track/2WfaOiMkCvy7F5fcp2zZ8L",
            "http://music.apple.com/song/1",
        ]:
            self.assert_backend_error(
                400,
                lambda song=song: self.viewer.order_song(song),
                "Paste the Apple Music share link of a song.",
            )
        offline = "The streamer is offline right now. Try again when they are live."
        self.assert_backend_error(404, lambda: self.viewer.order_song(SONG), offline)
        self.assert_backend_error(404, self.viewer.queue, offline)
        self.dashboard.save_settings()
        self.assert_backend_error(
            400,
            lambda: self.viewer.order_song(SONG),
            "The streamer no longer takes song requests. Reload the page.",
        )


class SongNotOnAppleMusic(SongRequestTestCase):
    """Refuse requests of songs the streamer's Apple Music can't find, before the viewer pays."""

    def run(self):
        self.skip_if_missing_capability(Capability.APPLE_MUSIC)
        self.connect_moblin()
        self.assert_backend_error(
            422,
            lambda: self.viewer.order_song(MISSING_SONG),
            "That song is not available on the streamer's Apple Music.",
        )
        find = self.moblin.backend_requests("find")[-1]
        self.assert_equal(find.body, {"title": "https://music.apple.com/song/99999999999?i=99999999999"})


class SongRequestPaid(SongRequestTestCase):
    """Request a song with PayPal, which adds it to the queue in Moblin and on the page."""

    def run(self):
        self.skip_if_missing_capability(Capability.APPLE_MUSIC)
        self.connect_moblin()
        self.moblin.clear_music()
        paywall = self.viewer.paywall()
        assert paywall is not None
        self.assert_equal(paywall["price"], 150)
        order_id = self.viewer.order_song(SONG, "Viewer")
        self.assert_equal(self.moblin.backend_requests("find")[-1].body, {"title": SONG_LINK})
        self.pay(order_id)
        self.assert_equal(self.capture(self.viewer.capture_song, order_id), {})
        self.assert_paid(order_id, "1.50")
        add = self.moblin.wait_for_backend_request("add", order_id)
        self.assert_equal(add.body, {"title": SONG_LINK, "user": "Viewer", "requestId": order_id})
        songs = self.moblin.music_status()["songs"]
        self.assert_equal([song["user"] for song in songs], ["Viewer"])

        def on_page() -> bool:
            return [song.get("user") for song in self.viewer.queue()["songs"]] == ["Viewer"]

        self.wait_until(on_page, "the song to be in the queue on the page")

    def teardown(self):
        if self.context.moblin is not None:
            self.moblin.clear_music()
        super().teardown()


class SongRequestWhenStreamerGoesOffline(SongRequestTestCase):
    """Do not take the money for a song when Moblin disconnects between ordering and paying."""

    def run(self):
        self.skip_if_missing_capability(Capability.APPLE_MUSIC)
        self.connect_moblin()
        order_id = self.viewer.order_song(OTHER_SONG)
        self.pay(order_id)
        self.moblin.disconnect_from_backend()
        self.wait_until(lambda: not self.dashboard.account()["settings"]["connected"], "Moblin to disconnect")
        self.assert_backend_error(
            404,
            lambda: self.capture(self.viewer.capture_song, order_id),
            "The streamer went offline, so your payment was not taken.",
        )
        self.assert_not_paid(order_id)


def tests(context: Context):
    return [
        TipNotApproved(context),
        InvalidTip(context),
        TipPaid(context),
        AnonymousTipPaid(context),
        TipDeliveredWhenMoblinConnects(context),
        TipDeliveredAfterBackendRestart(context),
        SpinNeedsWheel(context),
        SpinPaid(context),
        SpinWhenWheelIsGone(context),
        InvalidSongRequest(context),
        SongNotOnAppleMusic(context),
        SongRequestPaid(context),
        SongRequestWhenStreamerGoesOffline(context),
    ]
