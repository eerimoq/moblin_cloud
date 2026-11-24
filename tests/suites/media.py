from ..utils.context import Context
from ..utils.devices import RawDevice
from ..utils.devices import RecordingMoblin
from ..utils.test_case import TestCase

VIDEO = "dQw4w9WgXcQ"
LINK = f"https://youtu.be/{VIDEO}?t=43"
MEDIA = {"price": 300, "seconds": 60}
NOT_ASSISTANT = "Only the streamer and the assistants they turned on can do this."


class MediaTestCase(TestCase):
    """A streamer's page with YouTube videos viewers request, which the fake Moblin device picked for
    videos queues and plays until the test finishes them. The fake YouTube knows every video."""

    login = "media"

    def setup(self):
        self.dashboard = self.sign_in(self.login)
        self.page_id = self.dashboard.page_id()
        self.viewer = self.frontend.viewer(self.page_id)

    def teardown(self):
        self.backend.fake_youtube.reset()
        self.backend.fake_paypal.reset()
        if hasattr(self, "dashboard"):
            self.dashboard.change_settings(media=None, subMedia=None)

    def device(self, device_id: str = "MEDIA", media: bool = True) -> RecordingMoblin:
        device = RecordingMoblin(self.backend.url, self.dashboard.moblin_cloud_key(), device_id, media=media)
        device.start()
        self.wait_until(lambda: self.viewer.media()["connected"], "the device to connect")
        if media:
            self.wait_until(lambda: self.viewer.media()["available"], "the page to take videos")
        return device

    def paid(self, merchant_id: str, link: str = LINK, seconds: int | None = 30, **order) -> str:
        order_id = self.viewer.order_media(link, seconds, **order)
        self.context.paypal.pay_with_card(order_id, merchant_id)
        self.assert_equal(self.viewer.capture_media(order_id), {})
        return order_id

    def held(self) -> list[str]:
        return [video["id"] for video in self.dashboard.media_queue()["held"]]


class MediaNeedsPayments(MediaTestCase):
    """Refuse to turn paid videos on before PayPal or Stripe is connected, but not subscribers' videos."""

    login = "medianeedspay"

    def run(self):
        self.assert_backend_error(
            409, lambda: self.dashboard.change_settings(media=MEDIA), "connect PayPal or Stripe first"
        )
        media = self.viewer.media()
        self.assert_equal(
            media,
            {
                "media": None,
                "subMedia": None,
                "seconds": None,
                "moderated": False,
                "available": False,
                "connected": False,
            },
        )
        tiers = {"tier1": 1, "tier2": 0, "tier3": 0}
        self.dashboard.change_settings(subMedia=tiers)
        media = self.viewer.media()
        self.assert_equal(
            [media["media"], media["subMedia"], media["seconds"], media["moderated"]],
            [None, tiers, 60, True],
            "subscribers' videos need no payments and follow the default rules",
        )
        self.context.page_with_paypal(self.dashboard)
        settings = self.dashboard.change_settings(media=MEDIA, subMedia=tiers)
        self.assert_equal(settings["media"], {"enabled": True, "moderated": True, **MEDIA})
        media = self.viewer.media()
        self.assert_equal(media["media"]["price"], 300)
        self.assert_equal(media["media"]["merchantId"], settings["merchantId"])
        self.assert_equal(media["subMedia"], tiers)
        self.assert_equal([media["seconds"], media["moderated"]], [60, True])
        self.assert_backend_error(
            409,
            lambda: self.dashboard.change_settings(currency="EUR"),
            "turn off all tips before changing the currency",
        )
        self.dashboard.change_settings(
            media={"price": 300, "seconds": 20, "moderated": False, "enabled": False}
        )
        media = self.viewer.media()
        self.assert_equal(
            [media["media"], media["subMedia"], media["seconds"], media["moderated"]],
            [None, tiers, 20, False],
            "the rules of turned off paid videos still hold for subscribers",
        )
        self.dashboard.change_settings(subMedia=None)
        media = self.viewer.media()
        self.assert_equal([media["media"], media["subMedia"], media["seconds"]], [None, None, None])


class MediaInvalid(MediaTestCase):
    """Refuse video settings out of range, links that are not YouTube videos, videos YouTube does not
    let other sites play and lengths the streamer does not allow, all before any payment starts."""

    login = "mediainvalid"

    def run(self):
        settings = self.context.page_with_paypal(self.dashboard)
        for media in [
            {"price": 49, "seconds": 60},
            {"price": 10_000_001, "seconds": 60},
            {"price": 300, "seconds": 4},
            {"price": 300, "seconds": 601},
        ]:
            self.assert_backend_error(
                400,
                lambda media=media: self.dashboard.change_settings(media=media),
                "videos need a price of 0.50 to 100000 and 5 to 600 seconds",
            )
        self.assert_refused(lambda: self.dashboard.change_settings(media={"price": 300}))
        self.dashboard.change_settings(media=MEDIA)
        offline = "The streamer is not taking videos right now. Try again when they are live."
        self.assert_backend_error(404, lambda: self.viewer.order_media(LINK, 30), offline)
        with self.device(media=False) as device:
            self.assert_backend_error(
                409,
                lambda: self.viewer.order_media(LINK, 30),
                "The streamer is not taking videos right now.",
            )
            device.change_media(True)
            self.wait_until(lambda: self.viewer.media()["available"], "the page to take videos")
            not_youtube = "That is not a link to a YouTube video or a Twitch clip."
            for link in [
                "",
                "https://vimeo.com/123",
                "https://www.youtube.com/@someone",
                "https://www.youtube.com/clip/Ugkx-something",
                f"https://evil.example/watch?v={VIDEO}",
                f"https://youtube.com.evil.example/watch?v={VIDEO}",
                f"{VIDEO}x",
            ]:
                self.assert_backend_error(
                    400, lambda link=link: self.viewer.order_media(link, 30), not_youtube
                )
            for seconds in [0, 61]:
                self.assert_backend_error(
                    400,
                    lambda seconds=seconds: self.viewer.order_media(LINK, seconds),
                    "The streamer changed how long videos can be. Reload the page.",
                )
            self.assert_refused(lambda: self.viewer.order_media(LINK, -1))
            self.assert_backend_error(
                400,
                lambda: self.viewer.order_media(LINK, 30, price=200),
                "The streamer changed the price. Reload the page.",
            )
            self.assert_backend_error(400, lambda: self.viewer.order_media(LINK, 30, start=86401))
            self.assert_equal(self.backend.fake_youtube.lookups, [], "nothing was looked up yet")
            self.assert_backend_error(400, lambda: self.viewer.order_media(LINK, 30, message=201 * "x"))
            self.backend.fake_youtube.unavailable.add("private0000")
            self.assert_backend_error(
                400,
                lambda: self.viewer.order_media("https://youtu.be/private0000", 30),
                "That video does not exist or can't be played outside YouTube. Pick another one.",
            )
            self.backend.fake_youtube.down = True
            self.assert_backend_error(
                502, lambda: self.viewer.order_media(LINK, 30), "YouTube did not answer. Try again."
            )
            self.backend.fake_youtube.down = False
            order_id = self.viewer.order_media(LINK, 60, price=300)
            self.context.paypal.pay_with_card(order_id, settings["merchantId"])
            for capture in [self.viewer.capture_tip, self.viewer.capture_spin, self.viewer.capture_effect]:
                self.assert_backend_error(410, lambda capture=capture: capture(order_id))
            device.change_media(False)
            self.wait_until(lambda: not self.viewer.media()["available"], "the page to stop taking videos")
            self.assert_backend_error(
                404,
                lambda: self.viewer.capture_media(order_id),
                "The streamer stopped taking videos, so your tip was not taken.",
            )
            status = self.context.paypal.order(order_id, settings["merchantId"])["status"]
            self.assert_equal(status, "APPROVED", "no money was taken")
            self.assert_equal(self.held(), [])


class MediaModerated(MediaTestCase):
    """Hold a paid video until the streamer approves it, then let the picked Moblin device play it and
    show the dashboard what plays, what waits in Moblin and what is still held."""

    def __init__(self, context: Context, provider: str = "paypal"):
        super().__init__(context, f"{type(self).__name__}Stripe" if provider == "stripe" else None)
        self.provider = provider
        self.login = f"mediamoderated{provider}"

    def pay(self, settings: dict, link: str, seconds: int, user: str, message: str | None = None) -> str:
        order_id = self.viewer.order_media(link, seconds, 300, user, message, provider=self.provider)
        if self.provider == "stripe":
            self.context.stripe.pay_with_card(order_id, settings["stripeAccountId"])
        else:
            self.context.paypal.pay_with_card(order_id, settings["merchantId"])
        self.assert_equal(self.viewer.capture_media(order_id), {})
        return order_id

    def run(self):
        if self.provider == "stripe":
            settings = self.context.page_with_stripe(self.dashboard)
        else:
            settings = self.context.page_with_paypal(self.dashboard)
        self.dashboard.change_settings(media=MEDIA)
        with self.device("OTHER", media=False) as other, self.device("PLAYER") as player:
            first = self.pay(settings, LINK, 30, " Fan ", " Watch this ")
            second = self.pay(settings, "https://www.youtube.com/watch?v=aaaaaaaaaaa&t=1m5s", 60, "Other")
            self.assert_equal(self.viewer.capture_media(first), {})
            queue = self.dashboard.media_queue()
            self.assert_equal([queue["device"], queue["paused"], queue["playing"]], ["PLAYER", False, None])
            self.assert_equal(queue["queued"], [])
            self.assert_equal([video["id"] for video in queue["held"]], [first, second])
            held = queue["held"][0]
            del held["at"]
            self.assert_equal(
                held,
                {
                    "id": first,
                    "user": "Fan",
                    "message": "Watch this",
                    "video": VIDEO,
                    "start": 43,
                    "seconds": 30,
                    "title": f"Video {VIDEO}",
                    "platform": "youtube",
                    "amount": 300,
                    "currency": "USD",
                },
            )
            items = self.dashboard.recent("media")["items"]
            self.assert_equal(
                [(item["id"], item["status"]) for item in items], [(second, "held"), (first, "held")]
            )
            self.assert_equal([items[1]["video"], items[1]["title"]], [VIDEO, f"Video {VIDEO}"])
            self.assert_equal(player.kinds("youTube"), [], "nothing is played before it is approved")
            self.assert_backend_error(404, lambda: self.dashboard.approve_media("nope"))
            self.assert_equal(self.dashboard.approve_media(second), {})
            self.wait_until(lambda: player.tip_with_id(second) is not None, "the approved video in Moblin")
            self.assert_equal(
                player.tip_with_id(second),
                {
                    "name": "Other",
                    "message": "",
                    "requestId": second,
                    "kind": {"youTube": {"videoId": "aaaaaaaaaaa", "start": 65, "duration": 60}},
                },
            )
            self.assert_backend_error(
                404,
                lambda: self.dashboard.approve_media(second),
                "This video is no longer waiting for approval.",
            )
            self.assert_backend_error(404, lambda: self.dashboard.reject_media(second))
            self.assert_equal(self.dashboard.approve_media(first), {})
            self.wait_until(
                lambda: [video["id"] for video in self.dashboard.media_queue()["queued"]] == [first],
                "the second approved video to wait in Moblin",
            )
            queue = self.dashboard.media_queue()
            self.assert_equal(queue["playing"]["id"], second)
            self.assert_equal(queue["playing"]["title"], "Video aaaaaaaaaaa")
            self.assert_equal(queue["held"], [])
            self.assert_equal(other.kinds("youTube"), [], "only the picked device plays videos")
            self.dashboard.control_device(self.page_id, "PLAYER", {"setMoblinCloudMediaPaused": {"on": True}})
            self.wait_until(lambda: self.dashboard.media_queue()["paused"], "the queue to be paused")
            self.dashboard.control_device(self.page_id, "PLAYER", {"skipMoblinCloudMedia": {}})
            self.wait_until(
                lambda: self.dashboard.media_queue()["playing"] is None, "the playing video to be skipped"
            )
            self.assert_equal([video["id"] for video in self.dashboard.media_queue()["queued"]], [first])
            self.dashboard.control_device(
                self.page_id, "PLAYER", {"setMoblinCloudMediaPaused": {"on": False}}
            )
            self.wait_until(
                lambda: (self.dashboard.media_queue()["playing"] or {}).get("id") == first,
                "the next video to play",
            )
            player.finish_media()
            self.wait_until(lambda: self.dashboard.media_queue()["playing"] is None, "the queue to be empty")
        items = self.dashboard.recent("media")["items"]
        self.assert_equal(
            [(item["id"], item["status"], item["provider"]) for item in items],
            [(second, "delivered", self.provider), (first, "delivered", self.provider)],
        )


class MediaRejected(MediaTestCase):
    """Refund a rejected video, and keep it held while the provider refuses or loses the refund."""

    login = "mediarejected"

    def run(self):
        settings = self.context.page_with_paypal(self.dashboard)
        merchant_id = settings["merchantId"]
        self.dashboard.change_settings(media=MEDIA)
        with self.device() as device:
            order_id = self.paid(merchant_id, user="Troll")
            self.assert_equal(self.held(), [order_id])
            self.backend.fake_paypal.refunds = "refused"
            self.assert_backend_error(
                409,
                lambda: self.dashboard.reject_media(order_id),
                "PayPal refused to refund the tip, so the video can only be approved. "
                "Refund it at PayPal to reject it.",
            )
            self.backend.fake_paypal.refunds = "failed"
            self.assert_backend_error(
                502,
                lambda: self.dashboard.reject_media(order_id),
                "PayPal did not answer, so the tip was not refunded. Try again.",
            )
            self.assert_equal(self.held(), [order_id])
            self.backend.fake_paypal.refunds = "accepted"
            self.assert_equal(self.dashboard.reject_media(order_id), {})
            unit = self.context.paypal.order(order_id, merchant_id)["purchase_units"][0]
            self.assert_equal(unit["payments"]["captures"][0]["status"], "REFUNDED")
            self.assert_equal(self.held(), [])
            self.assert_equal(self.dashboard.recent("media")["items"], [])
            self.assert_backend_error(404, lambda: self.dashboard.reject_media(order_id))
            self.assert_backend_error(404, lambda: self.dashboard.approve_media(order_id))
            self.assert_backend_error(404, lambda: self.dashboard.resend(order_id))
            self.assert_equal(device.kinds("youTube"), [])


class MediaUnmoderated(MediaTestCase):
    """Send paid videos straight to Moblin when the streamer turned approval off, and again from the
    dashboard, and keep them for a Moblin that is away."""

    login = "mediaunmoderated"

    def run(self):
        settings = self.context.page_with_paypal(self.dashboard)
        self.dashboard.change_settings(media={**MEDIA, "moderated": False})
        self.assert_equal(self.viewer.media()["moderated"], False)
        with self.device() as device:
            first = self.paid(settings["merchantId"], seconds=20, start=5, user="Fan")
            self.wait_until(lambda: device.tip_with_id(first) is not None, "the video in Moblin")
            tip = device.tip_with_id(first)
            assert tip is not None
            self.assert_equal(tip["kind"], {"youTube": {"videoId": VIDEO, "start": 5, "duration": 20}})
            self.assert_equal(self.held(), [])
            self.wait_until(
                lambda: self.dashboard.recent("media")["items"][0]["status"] == "delivered",
                "the video to be delivered",
            )
            self.assert_equal(self.dashboard.resend(first), {})
            self.wait_until(lambda: len(device.kinds("youTube")) == 2, "the video to be sent again")
            self.assert_equal(device.kinds("youTube")[1]["requestId"], f"{first}-1")
            self.wait_until(
                lambda: [video["id"] for video in self.dashboard.media_queue()["queued"]] == [first],
                "the video sent again to wait in Moblin",
            )
            self.assert_equal(self.dashboard.media_queue()["playing"]["id"], first)
            second = self.viewer.order_media(LINK, 30)
            self.context.paypal.pay_with_card(second, settings["merchantId"])
        with self.device("OTHER", media=False):
            self.assert_backend_error(404, lambda: self.viewer.capture_media(second))
        with self.device() as device:
            self.assert_equal(self.viewer.capture_media(second), {})
            self.wait_until(lambda: device.tip_with_id(second) is not None, "the video in Moblin")


class MediaTestSend(MediaTestCase):
    """Send a free test video from the dashboard, the given one or the default one, without approval,
    once a Moblin takes videos."""

    login = "mediatest"

    def run(self):
        self.assert_backend_error(409, self.dashboard.test_media, "Connect Moblin first.")
        with self.device(media=False) as device:
            self.assert_backend_error(
                409,
                self.dashboard.test_media,
                "Pick a browser widget for media share in Moblin's Moblin Cloud settings first.",
            )
            device.change_media(True)
            self.wait_until(lambda: self.viewer.media()["available"], "the page to take videos")
            self.assert_backend_error(400, lambda: self.dashboard.test_media("https://vimeo.com/1"))
            self.assert_equal(self.dashboard.test_media("https://youtu.be/aaaaaaaaaaa?t=9", "Testing"), {})
            self.assert_equal(self.dashboard.test_media(), {})
            self.wait_until(lambda: len(device.kinds("youTube")) == 2, "the test videos in Moblin")
            videos = device.kinds("youTube")
            self.assert_equal([video["name"] for video in videos], ["mediatest", "mediatest"])
            self.assert_equal(
                [video["kind"]["youTube"] for video in videos],
                [
                    {"videoId": "aaaaaaaaaaa", "start": 9, "duration": 15},
                    {"videoId": VIDEO, "start": 0, "duration": 15},
                ],
            )
            self.assert_equal(self.dashboard.recent("media")["items"], [])
            self.assert_equal(self.held(), [])


class MediaForSubscribers(MediaTestCase):
    """Let subscribers request videos without paying, within their tier's videos, held for approval
    like paid ones and gone without a refund when rejected."""

    login = "mediasubs"

    def run(self):
        self.context.page_with_paypal(self.dashboard)
        self.dashboard.change_settings(media=MEDIA, subMedia={"tier1": 2, "tier2": 0, "tier3": 1})
        tier1 = self.frontend.subscriber(self.page_id, "mediafan1")
        self.assert_equal(
            tier1.media_allowance(),
            {"platform": "twitch", "name": "mediafan1", "tier": 1, "allowance": 2, "left": 2},
        )
        self.assert_backend_error(404, lambda: tier1.play_media(LINK, 30))
        with self.device() as device:
            self.assert_backend_error(400, lambda: tier1.play_media("https://vimeo.com/1", 30))
            self.assert_backend_error(400, lambda: tier1.play_media(LINK, 61))
            self.assert_equal(tier1.media_allowance()["left"], 2, "a refused video uses nothing")
            self.assert_equal(tier1.play_media(LINK, 30, " Enjoy "), {"left": 1})
            self.assert_equal(tier1.play_media("aaaaaaaaaaa", 10), {"left": 0})
            self.assert_backend_error(
                409, lambda: tier1.play_media(LINK, 30), "You have used all your videos this month."
            )
            self.assert_backend_error(
                403, lambda: self.frontend.subscriber(self.page_id, "mediafan2").play_media(LINK, 30)
            )
            self.assert_backend_error(
                403, lambda: self.frontend.subscriber(self.page_id, "mediafanviewer").play_media(LINK, 30)
            )
            held = self.dashboard.media_queue()["held"]
            self.assert_equal(
                [(video["user"], video.get("tier"), video.get("amount")) for video in held],
                [("mediafan1", 1, None), ("mediafan1", 1, None)],
            )
            self.assert_equal(held[0]["message"], "Enjoy")
            self.assert_equal(self.dashboard.reject_media(held[1]["id"]), {})
            self.assert_equal(self.dashboard.approve_media(held[0]["id"]), {})
            self.wait_until(lambda: len(device.kinds("youTube")) == 1, "the approved video in Moblin")
            self.assert_equal(device.kinds("youTube")[0]["name"], "mediafan1")
            self.assert_equal(tier1.media_allowance()["left"], 0, "a rejected video is not given back")
        items = self.dashboard.recent("media")["items"]
        self.assert_equal([(item["user"], item.get("tier")) for item in items], [("mediafan1", 1)])
        self.assert_true("provider" not in items[0], "subscribers paid with no provider")
        self.dashboard.change_settings(media={**MEDIA, "enabled": False})
        tier3 = self.frontend.subscriber(self.page_id, "mediafan3")
        with self.device():
            self.assert_equal(tier3.play_media(LINK, 60), {"left": 0}, "paid videos can be off")
            self.dashboard.change_settings(subMedia=None)
            self.assert_backend_error(400, lambda: tier3.play_media(LINK, 30))


class MediaClips(MediaTestCase):
    """Play Twitch clips whole, when they exist and are no longer than the streamer's longest video,
    and show the dashboard where each one is from."""

    login = "mediaclips"

    def run(self):
        settings = self.context.page_with_paypal(self.dashboard)
        self.dashboard.change_settings(media={"price": 300, "seconds": 30, "moderated": False})
        with self.device() as device:
            self.assert_backend_error(
                400,
                lambda: self.viewer.order_media("https://clips.twitch.tv/MissingClip"),
                "That clip does not exist. Pick another one.",
            )
            self.assert_backend_error(
                400,
                lambda: self.viewer.order_media("https://www.twitch.tv/someone/clip/LongClip"),
                "That clip is 60 seconds, and the streamer plays at most 30. Pick a shorter one.",
            )
            for link in [
                "https://www.twitch.tv/someone",
                "https://kick.com/someone/clips/clip_01M86Z1Z62360AM4D5KT55H4GS",
            ]:
                self.assert_backend_error(400, lambda link=link: self.viewer.order_media(link))
            clip = self.paid(settings["merchantId"], "https://www.twitch.tv/someone/clip/Funny-Clip_1", None)
            self.wait_until(lambda: device.tip_with_id(clip) is not None, "the clip in Moblin")
            tip = device.tip_with_id(clip)
            assert tip is not None
            self.assert_equal(tip["kind"], {"twitchClip": {"slug": "Funny-Clip_1", "duration": 29}})
            self.wait_until(
                lambda: (self.dashboard.media_queue()["playing"] or {}).get("id") == clip,
                "the clip to play in Moblin",
            )
            playing = self.dashboard.media_queue()["playing"]
            self.assert_equal(
                [playing["platform"], playing["title"], playing["link"], playing["thumbnail"]],
                [
                    "twitch",
                    "Clip Funny-Clip_1",
                    "https://clips.twitch.tv/Funny-Clip_1",
                    "https://static-cdn.jtvnw.net/Funny-Clip_1.jpg",
                ],
            )
            self.assert_equal(self.dashboard.test_media("https://clips.twitch.tv/Test-Clip"), {})
            self.wait_until(lambda: len(device.kinds("twitchClip")) == 2, "the test clip in Moblin")
        items = self.dashboard.recent("media")["items"]
        self.assert_equal([item["link"] for item in items], ["https://clips.twitch.tv/Funny-Clip_1"])


class MediaAssistants(MediaTestCase):
    """Let an assistant see, approve and reject the held videos, and nobody else."""

    login = "mediaassisted"

    def run(self):
        settings = self.context.page_with_paypal(self.dashboard)
        self.dashboard.change_settings(media=MEDIA)
        assistant = self.sign_in("mediamod")
        stranger = self.sign_in("mediastranger")
        self.dashboard.add_assistant("twitch", "mediamod")
        with self.device() as device:
            approved = self.paid(settings["merchantId"], user="Good")
            rejected = self.paid(settings["merchantId"], user="Bad")
            for call in [
                lambda: stranger.media_queue(self.page_id),
                lambda: stranger.approve_media(approved, self.page_id),
                lambda: stranger.reject_media(rejected, self.page_id),
                lambda: stranger.test_media(page_id=self.page_id),
            ]:
                self.assert_backend_error(403, call, NOT_ASSISTANT)
            queue = assistant.media_queue(self.page_id)
            self.assert_equal([video["user"] for video in queue["held"]], ["Good", "Bad"])
            self.assert_equal(assistant.reject_media(rejected, self.page_id), {})
            self.assert_equal(assistant.approve_media(approved, self.page_id), {})
            self.wait_until(lambda: device.tip_with_id(approved) is not None, "the approved video in Moblin")
            self.assert_equal(self.held(), [])
            self.assert_equal(device.tip_with_id(rejected), None)
            items = assistant.recent("media", page_id=self.page_id)["items"]
            self.assert_equal([item["id"] for item in items], [approved])
            self.assert_true("provider" not in items[0], "assistants don't see the provider")

    def teardown(self):
        if hasattr(self, "dashboard"):
            for assistant in self.dashboard.account()["twitchAssistants"]:
                self.dashboard.remove_assistant("twitch", assistant["id"])
        super().teardown()


class MediaStatusFromMoblin(MediaTestCase):
    """Take what Moblin says about its videos as it is: nothing from a Moblin that never says it, and
    only the requests of this page among the ones it names."""

    login = "mediastatus"

    def run(self):
        settings = self.context.page_with_paypal(self.dashboard)
        self.dashboard.change_settings(media={**MEDIA, "moderated": False})
        device = RawDevice(self.backend.port, self.dashboard.moblin_cloud_key(), "RAW")
        try:
            self.wait_until(lambda: self.viewer.media()["connected"], "the device to connect")
            self.assert_equal(self.viewer.media()["available"], False)
            self.assert_equal(self.dashboard.media_queue()["device"], None)
            device.push_media({"paused": False, "queued": []})
            self.wait_until(lambda: self.viewer.media()["available"], "the page to take videos")
            order_id = self.paid(settings["merchantId"])
            self.wait_until(lambda: len(device.tips()) == 1, "the video in Moblin")
            device.push_media(
                {"paused": True, "playing": "someone-elses", "queued": [order_id, "unknown", 7 * "x"]}
            )
            self.wait_until(lambda: self.dashboard.media_queue()["paused"], "the status of Moblin")
            queue = self.dashboard.media_queue()
            self.assert_equal(queue["playing"], None)
            self.assert_equal([video["id"] for video in queue["queued"]], [order_id])
            device.push_media(None)
            self.wait_until(lambda: not self.viewer.media()["available"], "the page to stop taking videos")
        finally:
            device.close()


def tests(context: Context):
    return [
        MediaNeedsPayments(context),
        MediaInvalid(context),
        MediaModerated(context, "paypal"),
        MediaModerated(context, "stripe"),
        MediaRejected(context),
        MediaUnmoderated(context),
        MediaTestSend(context),
        MediaForSubscribers(context),
        MediaClips(context),
        MediaAssistants(context),
        MediaStatusFromMoblin(context),
    ]
