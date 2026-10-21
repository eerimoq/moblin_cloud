"""Pretends to be the parts of PayPal's REST API the backend uses as a partner, plus what the tests
use to pay orders like a viewer.

Seller onboarding is a sign up link that connects a new made-up merchant at once and returns to the
return URL with the merchant ID, like PayPal. Orders are approved with confirm-payment-source, captured and looked up like in the
sandbox. Webhook signatures verify only when the transmission signature is "good".
"""

import base64
import binascii
import json
import logging
import re
import secrets
import string
import threading
from http.server import BaseHTTPRequestHandler
from http.server import ThreadingHTTPServer
from urllib.parse import parse_qs
from urllib.parse import urlencode
from urllib.parse import urlsplit

LOGGER = logging.getLogger(__name__)
CLIENT_ID = "fake-paypal-client-id"
CLIENT_SECRET = "fake-paypal-client-secret"
PARTNER_ID = "FAKEPARTNER01"
WEBHOOK_ID = "FAKE-WEBHOOK-ID"
TOKEN = "fake-paypal-token"


def random_id(length: int) -> str:
    return "".join(secrets.choice(string.ascii_uppercase + string.digits) for _ in range(length))


def error(name: str, issue: str) -> dict:
    return {"name": name, "details": [{"issue": issue}]}


def payer_of(assertion: str) -> str | None:
    try:
        payload = json.loads(base64.b64decode(assertion.split(".")[1]))
    except (IndexError, ValueError, binascii.Error):
        return None
    if payload.get("iss") != CLIENT_ID:
        return None
    return payload.get("payer_id")


class FakePayPal:
    def __init__(self, port: int = 0):
        self.url = ""
        self.config = {
            "client-id": CLIENT_ID,
            "client-secret": CLIENT_SECRET,
            "partner-id": PARTNER_ID,
            "webhook-id": WEBHOOK_ID,
        }
        self._port = port
        self._server: ThreadingHTTPServer | None = None
        self._lock = threading.Lock()
        self._referrals: dict[str, str] = {}
        self._merchants: dict[str, str] = {}
        self._orders: dict[str, dict] = {}

    def __enter__(self):
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()

    def start(self):
        self._server = ThreadingHTTPServer(("0.0.0.0", self._port), self._handler())
        self.url = f"http://localhost:{self._server.server_address[1]}"
        LOGGER.info("Fake PayPal at %s", self.url)
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
                if parts.path == "/signup":
                    self.sign_up(query.get("tracking_id", ""))
                    return
                if not self.authorized():
                    return
                integrations = f"/v1/customer/partners/{PARTNER_ID}/merchant-integrations"
                order = re.match(r"^/v2/checkout/orders/([A-Z0-9]+)$", parts.path)
                if parts.path == integrations:
                    self.integration(query.get("tracking_id", ""))
                elif parts.path.startswith(f"{integrations}/"):
                    self.seller_status(parts.path.removeprefix(f"{integrations}/"))
                elif order:
                    self.order(order[1])
                else:
                    self.json(error("RESOURCE_NOT_FOUND", "INVALID_RESOURCE_ID"), 404)

            def do_POST(self):  # pylint: disable=invalid-name
                length = int(self.headers.get("content-length", "0"))
                body = self.rfile.read(length).decode()
                path = urlsplit(self.path).path
                if path == "/v1/oauth2/token":
                    self.token()
                    return
                if not self.authorized():
                    return
                try:
                    data = json.loads(body) if body else {}
                except ValueError:
                    self.json(error("INVALID_REQUEST", "MALFORMED_REQUEST_JSON"), 400)
                    return
                order = re.match(r"^/v2/checkout/orders/([A-Z0-9]+)/(confirm-payment-source|capture)$", path)
                if path == "/v2/customer/partner-referrals":
                    self.referral(data)
                elif path == "/v1/notifications/verify-webhook-signature":
                    good = data.get("webhook_id") == WEBHOOK_ID and data.get("transmission_sig") == "good"
                    self.json({"verification_status": "SUCCESS" if good else "FAILURE"})
                elif path == "/v2/checkout/orders":
                    self.create_order(data)
                elif order and order[2] == "confirm-payment-source":
                    self.confirm(order[1])
                elif order:
                    self.capture(order[1])
                else:
                    self.json(error("RESOURCE_NOT_FOUND", "INVALID_RESOURCE_ID"), 404)

            def authorized(self) -> bool:
                if self.headers.get("authorization", "") != f"Bearer {TOKEN}":
                    self.json({"error": "invalid_token", "error_description": "bad token"}, 401)
                    return False
                return True

            def token(self):
                expected = base64.b64encode(f"{CLIENT_ID}:{CLIENT_SECRET}".encode()).decode()
                if self.headers.get("authorization", "") != f"Basic {expected}":
                    self.json({"error": "invalid_client", "error_description": "bad credentials"}, 401)
                    return
                self.json({"access_token": TOKEN, "token_type": "Bearer", "expires_in": 32400})

            def referral(self, data: dict):
                tracking_id = data.get("tracking_id", "")
                return_url = data.get("partner_config_override", {}).get("return_url")
                if not tracking_id or return_url is None:
                    self.json(error("INVALID_REQUEST", "MISSING_REQUIRED_PARAMETER"), 400)
                    return
                with fake._lock:  # pylint: disable=protected-access
                    fake._referrals[tracking_id] = return_url  # pylint: disable=protected-access
                href = f"{fake.url}/signup?{urlencode({'tracking_id': tracking_id})}"
                self.json({"links": [{"href": href, "rel": "action_url", "method": "GET"}]}, 201)

            def sign_up(self, tracking_id: str):
                with fake._lock:  # pylint: disable=protected-access
                    return_url = fake._referrals.pop(tracking_id, None)  # pylint: disable=protected-access
                    merchant_id = random_id(13)
                    if return_url is not None:
                        fake._merchants[merchant_id] = tracking_id  # pylint: disable=protected-access
                if return_url is None:
                    self.respond(404, "text/plain", b"No such referral.")
                    return
                separator = "&" if "?" in return_url else "?"
                params = urlencode({"merchantId": tracking_id, "merchantIdInPayPal": merchant_id})
                self.send_response(302)
                self.send_header("location", f"{return_url}{separator}{params}")
                self.send_header("content-length", "0")
                self.end_headers()

            def integration(self, tracking_id: str):
                with fake._lock:  # pylint: disable=protected-access
                    merchants = [
                        merchant
                        for merchant, tracking in fake._merchants.items()  # pylint: disable=protected-access
                        if tracking == tracking_id
                    ]
                if not merchants:
                    self.json(error("RESOURCE_NOT_FOUND", "INVALID_RESOURCE_ID"), 404)
                    return
                self.json({"merchant_id": merchants[0], "tracking_id": tracking_id})

            def seller_status(self, merchant_id: str):
                with fake._lock:  # pylint: disable=protected-access
                    tracking_id = fake._merchants.get(merchant_id)  # pylint: disable=protected-access
                if tracking_id is None:
                    self.json(error("RESOURCE_NOT_FOUND", "INVALID_RESOURCE_ID"), 404)
                    return
                self.json(
                    {
                        "merchant_id": merchant_id,
                        "tracking_id": tracking_id,
                        "payments_receivable": True,
                        "primary_email_confirmed": True,
                        "oauth_integrations": [
                            {
                                "integration_type": "OAUTH_THIRD_PARTY",
                                "oauth_third_party": [
                                    {
                                        "partner_client_id": CLIENT_ID,
                                        "scopes": [
                                            "https://uri.paypal.com/services/payments/realtime-payment"
                                        ],
                                    }
                                ],
                            }
                        ],
                    }
                )

            def create_order(self, data: dict):
                payer = payer_of(self.headers.get("paypal-auth-assertion", ""))
                units = data.get("purchase_units", [])
                if len(units) != 1 or "amount" not in units[0]:
                    self.json(error("INVALID_REQUEST", "MISSING_REQUIRED_PARAMETER"), 400)
                    return
                payee = units[0].get("payee", {}).get("merchant_id")
                with fake._lock:  # pylint: disable=protected-access
                    known = payee in fake._merchants  # pylint: disable=protected-access
                if payer is None or payer != payee or not known:
                    self.json(error("UNPROCESSABLE_ENTITY", "PAYEE_ACCOUNT_INVALID"), 422)
                    return
                order_id = random_id(17)
                order = {
                    "id": order_id,
                    "intent": data.get("intent"),
                    "status": "CREATED",
                    "purchase_units": [{"reference_id": "default", **units[0]}],
                }
                with fake._lock:  # pylint: disable=protected-access
                    fake._orders[order_id] = order  # pylint: disable=protected-access
                self.json({"id": order_id, "status": "CREATED"}, 201)

            def found(self, order_id: str) -> dict | None:
                with fake._lock:  # pylint: disable=protected-access
                    order = fake._orders.get(order_id)  # pylint: disable=protected-access
                if order is None:
                    self.json(error("RESOURCE_NOT_FOUND", "INVALID_RESOURCE_ID"), 404)
                    return None
                payer = payer_of(self.headers.get("paypal-auth-assertion", ""))
                if payer != order["purchase_units"][0]["payee"]["merchant_id"]:
                    self.json(error("NOT_AUTHORIZED", "PERMISSION_DENIED"), 403)
                    return None
                return order

            def order(self, order_id: str):
                order = self.found(order_id)
                if order is not None:
                    with fake._lock:  # pylint: disable=protected-access
                        self.json(order)

            def confirm(self, order_id: str):
                order = self.found(order_id)
                if order is None:
                    return
                with fake._lock:  # pylint: disable=protected-access
                    if order["status"] == "CREATED":
                        order["status"] = "APPROVED"
                    self.json(order)

            def capture(self, order_id: str):
                order = self.found(order_id)
                if order is None:
                    return
                with fake._lock:  # pylint: disable=protected-access
                    if order["status"] == "COMPLETED":
                        self.json(error("UNPROCESSABLE_ENTITY", "ORDER_ALREADY_CAPTURED"), 422)
                        return
                    if order["status"] != "APPROVED":
                        self.json(error("UNPROCESSABLE_ENTITY", "ORDER_NOT_APPROVED"), 422)
                        return
                    order["status"] = "COMPLETED"
                    unit = order["purchase_units"][0]
                    unit["payments"] = {
                        "captures": [
                            {
                                "id": random_id(17),
                                "status": "COMPLETED",
                                "amount": unit["amount"],
                                "final_capture": True,
                            }
                        ]
                    }
                    self.json(order, 201)

            def json(self, body: dict, status: int = 200):
                self.respond(status, "application/json", json.dumps(body).encode())

            def respond(self, status: int, content_type: str, body: bytes):
                self.send_response(status)
                self.send_header("content-type", content_type)
                self.send_header("content-length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

        return Handler
