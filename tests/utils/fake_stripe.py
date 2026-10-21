"""Pretends to be the parts of Stripe's API the backend uses as a Connect platform, plus what the tests
use to pay payments like a viewer.

Connected accounts take payments once their onboarding link has been opened, which returns to the
return URL at once. Payments are confirmed with the test card pm_card_visa, captured, canceled and
looked up like in test mode.
"""

import base64
import json
import logging
import re
import secrets
import string
import threading
import time
from http.server import BaseHTTPRequestHandler
from http.server import ThreadingHTTPServer
from urllib.parse import parse_qs
from urllib.parse import urlencode
from urllib.parse import urlsplit

LOGGER = logging.getLogger(__name__)
SECRET_KEY = "sk_test_fake"
PUBLISHABLE_KEY = "pk_test_fake"
TEST_CARD = "pm_card_visa"


def random_id(prefix: str, length: int) -> str:
    return prefix + "".join(secrets.choice(string.ascii_letters + string.digits) for _ in range(length))


def error(code: str, message: str, payment: dict | None = None) -> dict:
    body: dict = {"type": "invalid_request_error", "code": code, "message": message}
    if payment is not None:
        body["payment_intent"] = payment
    return {"error": body}


class FakeStripe:
    def __init__(self, port: int = 0):
        self.url = ""
        self.config = {"secret-key": SECRET_KEY, "publishable-key": PUBLISHABLE_KEY}
        self._port = port
        self._server: ThreadingHTTPServer | None = None
        self._lock = threading.Lock()
        self._accounts: dict[str, dict] = {}
        self._payments: dict[str, dict] = {}

    def __enter__(self):
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()

    def start(self):
        self._server = ThreadingHTTPServer(("0.0.0.0", self._port), self._handler())
        self.url = f"http://localhost:{self._server.server_address[1]}"
        LOGGER.info("Fake Stripe at %s", self.url)
        threading.Thread(target=self._server.serve_forever, daemon=True).start()

    def stop(self):
        if self._server is not None:
            self._server.shutdown()
            self._server.server_close()
            self._server = None

    def _handler(self):
        fake = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, format, *args):  # pylint: disable=redefined-builtin
                LOGGER.debug(format, *args)

            def do_GET(self):  # pylint: disable=invalid-name
                parts = urlsplit(self.path)
                query = {key: values[0] for key, values in parse_qs(parts.query).items()}
                onboarding = re.match(r"^/setup/(acct_\w+)$", parts.path)
                if onboarding:
                    self.onboard(onboarding[1], query.get("return_url", ""))
                    return
                if not self.authorized():
                    return
                account = re.match(r"^/v2/core/accounts/(acct_\w+)$", parts.path)
                payment = re.match(r"^/v1/payment_intents/(pi_\w+)$", parts.path)
                if account:
                    self.account(account[1])
                elif payment:
                    with fake._lock:  # pylint: disable=protected-access
                        found = self.payment(payment[1])
                        if found is not None:
                            self.json(found)
                else:
                    self.json(error("resource_missing", "Unrecognized request URL."), 404)

            def do_POST(self):  # pylint: disable=invalid-name
                length = int(self.headers.get("content-length", "0"))
                body = self.rfile.read(length).decode()
                path = urlsplit(self.path).path
                if not self.authorized():
                    return
                if path.startswith("/v2/"):
                    try:
                        data = json.loads(body) if body else {}
                    except ValueError:
                        self.json(error("parameter_invalid", "Invalid JSON."), 400)
                        return
                    if path == "/v2/core/accounts":
                        self.create_account(data)
                    elif path == "/v2/core/account_links":
                        self.account_link(data)
                    else:
                        self.json(error("resource_missing", "Unrecognized request URL."), 404)
                    return
                form = {key: values[0] for key, values in parse_qs(body).items()}
                action = re.match(r"^/v1/payment_intents/(pi_\w+)/(confirm|capture|cancel)$", path)
                if path == "/v1/payment_intents":
                    self.create_payment(form)
                elif action:
                    with fake._lock:  # pylint: disable=protected-access
                        self.change_payment(action[1], action[2], form)
                else:
                    self.json(error("resource_missing", "Unrecognized request URL."), 404)

            def authorized(self) -> bool:
                header = self.headers.get("authorization", "")
                basic = base64.b64encode(f"{SECRET_KEY}:".encode()).decode()
                if header not in [f"Bearer {SECRET_KEY}", f"Basic {basic}"]:
                    self.json(
                        {"error": {"type": "invalid_request_error", "message": "Invalid API Key."}}, 401
                    )
                    return False
                return True

            def create_account(self, data: dict):
                account = {
                    "id": random_id("acct_", 16),
                    "display_name": data.get("display_name"),
                    "country": data.get("identity", {}).get("country"),
                    "ready": False,
                }
                with fake._lock:  # pylint: disable=protected-access
                    fake._accounts[account["id"]] = account  # pylint: disable=protected-access
                self.json({"id": account["id"], "object": "v2.core.account"})

            def account_link(self, data: dict):
                account = data.get("account", "")
                onboarding = data.get("use_case", {}).get("account_onboarding", {})
                with fake._lock:  # pylint: disable=protected-access
                    known = account in fake._accounts  # pylint: disable=protected-access
                if not known or "return_url" not in onboarding:
                    self.json(error("resource_missing", "No such account."), 404)
                    return
                url = f"{fake.url}/setup/{account}?{urlencode({'return_url': onboarding['return_url']})}"
                self.json(
                    {"object": "v2.core.account_link", "url": url, "expires_at": int(time.time()) + 300}
                )

            def onboard(self, account_id: str, return_url: str):
                with fake._lock:  # pylint: disable=protected-access
                    account = fake._accounts.get(account_id)  # pylint: disable=protected-access
                    if account is not None:
                        account["ready"] = True
                if account is None or not return_url:
                    self.respond(404, "text/plain", b"No such account.")
                    return
                self.send_response(302)
                self.send_header("location", return_url)
                self.send_header("content-length", "0")
                self.end_headers()

            def account(self, account_id: str):
                with fake._lock:  # pylint: disable=protected-access
                    account = fake._accounts.get(account_id)  # pylint: disable=protected-access
                if account is None:
                    self.json(error("resource_missing", "No such account."), 404)
                    return
                status = "active" if account["ready"] else "pending"
                self.json(
                    {
                        "id": account_id,
                        "object": "v2.core.account",
                        "display_name": account["display_name"],
                        "configuration": {
                            "merchant": {"capabilities": {"card_payments": {"status": status}}}
                        },
                    }
                )

            def connected(self) -> str | None:
                account_id = self.headers.get("stripe-account", "")
                with fake._lock:  # pylint: disable=protected-access
                    account = fake._accounts.get(account_id)  # pylint: disable=protected-access
                if account is None:
                    self.json(error("account_invalid", "No such account."), 403)
                    return None
                return account_id

            def create_payment(self, form: dict):
                account_id = self.connected()
                if account_id is None:
                    return
                with fake._lock:  # pylint: disable=protected-access
                    ready = fake._accounts[account_id]["ready"]  # pylint: disable=protected-access
                if not ready:
                    self.json(error("account_invalid", "The account can't take card payments."), 400)
                    return
                try:
                    amount = int(form.get("amount", ""))
                except ValueError:
                    self.json(error("parameter_invalid_integer", "Invalid integer: amount"), 400)
                    return
                payment_id = random_id("pi_", 24)
                payment = {
                    "id": payment_id,
                    "object": "payment_intent",
                    "account": account_id,
                    "amount": amount,
                    "amount_received": 0,
                    "currency": form.get("currency"),
                    "capture_method": form.get("capture_method", "automatic"),
                    "client_secret": random_id(f"{payment_id}_secret_", 24),
                    "description": form.get("description"),
                    "metadata": {
                        key.removeprefix("metadata[").removesuffix("]"): value
                        for key, value in form.items()
                        if key.startswith("metadata[")
                    },
                    "status": "requires_payment_method",
                }
                with fake._lock:  # pylint: disable=protected-access
                    fake._payments[payment_id] = payment  # pylint: disable=protected-access
                    self.json(self.public(payment))

            def payment(self, payment_id: str) -> dict | None:
                payment = fake._payments.get(payment_id)  # pylint: disable=protected-access
                if payment is None or payment["account"] != self.headers.get("stripe-account", ""):
                    self.json(error("resource_missing", f"No such payment_intent: '{payment_id}'"), 404)
                    return None
                return self.public(payment)

            def public(self, payment: dict) -> dict:
                return {key: value for key, value in payment.items() if key != "account"}

            def change_payment(self, payment_id: str, action: str, form: dict):
                if self.payment(payment_id) is None:
                    return
                payment = fake._payments[payment_id]  # pylint: disable=protected-access
                status = payment["status"]
                if action == "confirm" and status in ["requires_payment_method", "requires_confirmation"]:
                    if form.get("payment_method") != TEST_CARD:
                        self.json(error("resource_missing", "No such PaymentMethod."), 400)
                        return
                    payment["status"] = (
                        "requires_capture" if payment["capture_method"] == "manual" else "succeeded"
                    )
                    if payment["status"] == "succeeded":
                        payment["amount_received"] = payment["amount"]
                elif action == "capture" and status == "requires_capture":
                    payment["status"] = "succeeded"
                    payment["amount_received"] = payment["amount"]
                elif action == "cancel" and status not in ["succeeded", "canceled"]:
                    payment["status"] = "canceled"
                else:
                    message = f"This PaymentIntent's status is {status}, so it can't be {action}ed."
                    self.json(error("payment_intent_unexpected_state", message, self.public(payment)), 400)
                    return
                self.json(self.public(payment))

            def json(self, body: dict, status: int = 200):
                self.respond(status, "application/json", json.dumps(body).encode())

            def respond(self, status: int, content_type: str, body: bytes):
                self.send_response(status)
                self.send_header("content-type", content_type)
                self.send_header("content-length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

        return Handler
