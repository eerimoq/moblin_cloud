from .suites import access
from .suites import account
from .suites import deliveries
from .suites import devices
from .suites import effects
from .suites import events
from .suites import images
from .suites import kick
from .suites import lifecycle
from .suites import moderators
from .suites import pages
from .suites import payments
from .suites import paypal
from .suites import persistence
from .suites import received
from .suites import remote_control
from .suites import robustness
from .suites import settings
from .suites import stripe
from .suites import subscribers
from .utils.runner import create_parser
from .utils.runner import run


def create_suites(context):
    return [
        account.tests(context),
        remote_control.tests(context),
        paypal.tests(context),
        stripe.tests(context),
        payments.tests(context),
        subscribers.tests(context),
        moderators.tests(context),
        persistence.tests(context),
        devices.tests(context),
        effects.tests(context),
        events.tests(context),
        received.tests(context),
        images.tests(context),
        pages.tests(context),
        access.tests(context),
        settings.tests(context),
        deliveries.tests(context),
        robustness.tests(context),
        lifecycle.tests(context),
        kick.tests(context),
    ]


def main():
    parser = create_parser("Run the backend system tests.")
    run("test", parser, create_suites)


main()
