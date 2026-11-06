from .backend import Backend
from .frontend import Dashboard
from .frontend import Frontend
from .moblin import Moblin
from .paypal import PayPal
from .stripe import Stripe


class Context:
    """Everything the tests share: the backend, the frontend talking to it and Moblin."""

    def __init__(self, backend: Backend, moblin: Moblin):
        self.backend = backend
        self.frontend = Frontend(backend.url)
        self.moblin = moblin
        self.paypal = PayPal(backend.fake_paypal.url)
        self.stripe = Stripe(backend.fake_stripe.url)

    def sign_in(self, login: str = "streamer") -> Dashboard:
        return self.frontend.fake_sign_in(login)

    def page_with_paypal(self, dashboard: Dashboard) -> dict:
        """Connects a PayPal seller to the signed in streamer's page, if not already."""
        account = dashboard.account()
        if account["settings"]["merchantId"] is not None:
            return account["settings"]
        self.paypal.onboard_seller(dashboard.connect_paypal())
        return dashboard.check_paypal()

    def page_with_stripe(self, dashboard: Dashboard) -> dict:
        """Gives the signed in streamer's page a Stripe account that takes payments, if not already."""
        account = dashboard.account()
        if account["settings"]["stripeAccountId"] is not None:
            return account["settings"]
        self.stripe.onboard_account(dashboard.connect_stripe())
        return dashboard.check_stripe()
