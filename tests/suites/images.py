import io

import requests
from PIL import Image

from ..utils.context import Context
from ..utils.events import EventStream
from ..utils.fake_twitch import user_id_of
from ..utils.test_case import TestCase


def image(width: int, height: int, format_: str = "PNG", color: tuple = (200, 40, 40)) -> bytes:
    picture = Image.new("RGB", (width, height), color)
    for x in range(width // 2 - 5, width // 2 + 5):
        for y in range(height):
            picture.putpixel((x, y), (255, 255, 255))
    output = io.BytesIO()
    picture.save(output, format_)
    return output.getvalue()


class ImagesTestCase(TestCase):
    """Banner images of premium streamers' pages. admin is an admin, and so premium."""

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.premium = self.sign_in("admin")
        self.page_id = self.premium.page_id()
        self.viewer = self.frontend.viewer(self.page_id)

    def teardown(self):
        if hasattr(self, "premium"):
            self.premium.remove_image()

    def served(self, version: str) -> requests.Response:
        return requests.get(f"{self.backend.url}/pages/{self.page_id}/image?v={version}", timeout=10)


class ImagesUpload(ImagesTestCase):
    """Crop an uploaded banner to 3:1 around the center, re-encode it as a 1500×500 JPEG and serve it for a year."""

    def run(self):
        with EventStream(self.frontend, f"/pages/{self.page_id}/events") as events:
            settings = self.premium.upload_image(image(1200, 1200))
            version = settings["image"]
            self.assert_true(isinstance(version, str) and version != "", settings)
            self.assert_equal(self.viewer.profile()["image"], version)
            self.assert_equal(self.premium.account()["settings"]["image"], version)
            events.wait_for("page", 5)
        response = self.served(version)
        self.assert_equal(response.status_code, 200)
        self.assert_equal(response.headers["content-type"], "image/jpeg")
        cache = response.headers.get("cache-control", "")
        self.assert_true("immutable" in cache and "max-age=31536000" in cache, cache)
        banner = Image.open(io.BytesIO(response.content))
        self.assert_equal([banner.format, banner.size], ["JPEG", (1500, 500)])
        self.assert_equal(banner.getexif(), {}, "nothing but pixels is kept")
        center = banner.getpixel((750, 250))
        assert isinstance(center, tuple)
        self.assert_true(all(channel > 200 for channel in center), f"the center stays white {center}")
        self.assert_equal(
            self.premium.upload_image(image(1200, 1200))["image"], version, "same image, same version"
        )
        for width, height, format_ in [
            (600, 200, "PNG"),
            (4000, 4000, "PNG"),
            (600, 4000, "JPEG"),
            (4000, 600, "WEBP"),
            (900, 300, "GIF"),
        ]:
            version = self.premium.upload_image(image(width, height, format_, (10, 90, 200)))["image"]
            banner = Image.open(io.BytesIO(self.served(version).content))
            self.assert_equal(banner.size, (1500, 500), f"{width}×{height} {format_}")
        self.assert_equal(self.premium.remove_image()["image"], None)
        self.assert_equal(self.viewer.profile()["image"], None)
        self.assert_equal(self.served(version).status_code, 404)


class ImagesRefused(ImagesTestCase):
    """Refuse banners too small, too large, not images or from streamers who are not premium."""

    def run(self):
        for data in [
            image(599, 600),
            image(4001, 1000),
            image(1000, 4001),
            b"",
            b"not an image",
            image(1200, 600)[:200],
            b"<svg xmlns='http://www.w3.org/2000/svg' width='3000' height='1000'/>",
        ]:
            self.assert_refused(lambda data=data: self.premium.upload_image(data))
        self.assert_refused(lambda: self.premium.upload_image(image(1500, 500) + b"\0" * (2 * 1024 * 1024)))
        self.assert_equal(self.viewer.profile()["image"], None)
        normal = self.sign_in("normalimage")
        self.assert_backend_error(403, lambda: normal.upload_image(image(1500, 500)))
        self.assert_equal(normal.remove_image()["image"], None)
        self.assert_equal(
            requests.get(f"{self.backend.url}/pages/{normal.page_id()}/image", timeout=10).status_code, 404
        )


class ImagesByModerators(ImagesTestCase):
    """Let a moderator change the banner of a premium streamer, but not of others."""

    def run(self):
        moderator = self.sign_in("imagemod")
        self.premium.add_moderator("twitch", "imagemod")
        try:
            settings = moderator.upload_image(image(1500, 500), self.page_id)
            self.assert_equal(self.viewer.profile()["image"], settings["image"])
            self.assert_equal(moderator.moderated_remove_image(self.page_id)["image"], None)
        finally:
            self.premium.remove_moderator("twitch", user_id_of("imagemod"))
        self.assert_backend_error(403, lambda: moderator.upload_image(image(1500, 500), self.page_id))
        normal = self.sign_in("imagenormal")
        normal.add_moderator("twitch", "imagemod")
        self.assert_backend_error(403, lambda: moderator.upload_image(image(1500, 500), normal.page_id()))
        normal.remove_moderator("twitch", user_id_of("imagemod"))


def tests(context: Context):
    return [
        ImagesUpload(context),
        ImagesRefused(context),
        ImagesByModerators(context),
    ]
