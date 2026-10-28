from collections.abc import Callable

import systest
from systest import wait_until

from .context import Context
from .frontend import BackendError
from .frontend import Dashboard


class TestCase(systest.TestCase):
    def __init__(self, context: Context, name: str | None = None):
        super().__init__(name)
        self.context = context
        self.backend = context.backend
        self.frontend = context.frontend

    def skip_if_no_twitch(self):
        if self.context.config.twitch() is None:
            raise systest.TestCaseSkippedError("No [twitch] in the configuration.")

    def skip_if_real_twitch_sign_in(self):
        if self.context.config.twitch_sign_in() == "real":
            raise systest.TestCaseSkippedError("Needs made-up Twitch users.")

    def skip_if_no_moblin(self):
        if self.context.moblin is None:
            raise systest.TestCaseSkippedError("No Moblin device given with --device.")

    def skip_if_not_interactive(self):
        if not self.context.interactive:
            raise systest.TestCaseSkippedError("Not interactive.")

    def sign_in(self, login: str = "streamer") -> Dashboard:
        return self.context.sign_in(login)

    def assert_backend_error(self, status: int, call: Callable[..., object], text: str | None = None) -> str:
        try:
            call()
        except BackendError as error:
            self.assert_equal(error.status, status, f"Answer {error.text!r}")
            if text is not None:
                self.assert_equal(error.text, text)
            return error.text
        raise Exception(f"Expected the backend to answer {status}.")

    def assert_refused(self, call: Callable[..., object]) -> int:
        """Checks that the backend refuses the call as the caller's mistake, with any 4xx."""
        try:
            call()
        except BackendError as error:
            self.assert_true(400 <= error.status < 500, f"Answer {error.status} {error.text!r}")
            return error.status
        raise Exception("Expected the backend to refuse.")

    def wait_until(
        self, check: Callable[[], bool], description: str = "condition to be true", timeout: float = 60
    ):
        wait_until(check, description, timeout=timeout)
