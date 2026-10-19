from .suites import account
from .suites import payments
from .suites import paypal
from .suites import persistence
from .suites import remote_control
from .suites import streamers
from .suites import stripe
from .suites import subscribers
from .utils.runner import create_parser
from .utils.runner import run


def create_suites(context):
    return [
        streamers.tests(context),
        account.tests(context),
        remote_control.tests(context),
        paypal.tests(context),
        stripe.tests(context),
        payments.tests(context),
        subscribers.tests(context),
        persistence.tests(context),
    ]


def main():
    parser = create_parser("Run the backend system tests.")
    run("test", parser, create_suites)


main()
