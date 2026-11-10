from collections.abc import Callable

import systest

from .context import Context
from .frontend import BackendError
from .frontend import Dashboard
from .utils import wait_until


class TestCase(systest.TestCase):
    def __init__(self, context: Context, name: str | None = None):
        super().__init__(name)
        self.context = context
        self.backend = context.backend
        self.frontend = context.frontend

    def sign_in(self, login: str = "streamer") -> Dashboard:
        return self.context.sign_in(login)

    def account_id_of(self, admin: Dashboard, login: str) -> str:
        found = admin.search_accounts(login)
        return next(
            account["id"]
            for account in found
            if any(identity["login"] == login for identity in account["identities"])
        )

    def sign_in_premium(self, login: str) -> Dashboard:
        dashboard = self.sign_in(login)
        admin = self.sign_in("admin")
        admin.set_premium(self.account_id_of(admin, login), True)
        return dashboard

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
