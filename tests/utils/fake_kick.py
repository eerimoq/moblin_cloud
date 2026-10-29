#!/usr/bin/env python3
"""Pretends to be Kick's sign in (OAuth with PKCE), the public API endpoints it needs and the website
API's channel user lookup, for signing in as made-up users.

Any login signs in. Its Kick user id is a number of 8 digits made from the login, like a real one,
and its channel slug is the login. A login ending in 1, 2 or 3 is a subscriber of everyone, like on
the fake Twitch. Any login can be added as a Kick assistant.
"""

import argparse
import base64
import hashlib
import html
import json
import logging
import re
import threading
from http.server import BaseHTTPRequestHandler
from http.server import ThreadingHTTPServer
from urllib.parse import parse_qs
from urllib.parse import urlencode
from urllib.parse import urlsplit

LOGGER = logging.getLogger(__name__)
LOGIN_RE = re.compile(r"^[a-z0-9_-]{1,25}$")
USERS = [
    ("admin", "an admin, and so premium"),
    ("premium", "a premium account"),
    ("normal", "neither admin nor premium"),
    ("sub1", "a subscriber"),
]
APP_TOKEN = "app-token"

PAGE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Fake Kick</title>
<style>
body {{ font-family: system-ui, sans-serif; background: #0b0e0f; color: #f5f5f5; display: grid; place-items: center; min-height: 100vh; margin: 0; }}
main {{ background: #141a1c; padding: 32px; border-radius: 12px; width: min(420px, calc(100vw - 32px)); box-sizing: border-box; }}
h1 {{ color: #53fc18; margin-top: 0; }}
form {{ margin: 0; }}
.users {{ display: flex; flex-direction: column; gap: 6px; }}
.users button {{ display: flex; justify-content: space-between; gap: 16px; width: 100%; text-align: left; font: inherit; padding: 12px 16px; border-radius: 8px; border: 0; background: #22292c; color: inherit; cursor: pointer; }}
.users button:hover {{ background: #53fc18; color: #0b0e0f; }}
.users button:hover small {{ color: #0b0e0f; }}
.users button b {{ font-weight: 600; }}
.users small {{ color: #a8b0b3; }}
.other {{ display: flex; gap: 8px; margin-top: 20px; }}
input {{ flex: 1; min-width: 0; font: inherit; padding: 10px; border-radius: 6px; border: 1px solid #3a4245; background: #0b0e0f; color: inherit; }}
.other button {{ font: inherit; padding: 10px 16px; border-radius: 6px; border: 0; background: #3a4245; color: white; cursor: pointer; }}
p {{ color: #a8b0b3; font-size: 14px; }}
a {{ color: #a8b0b3; }}
</style>
</head>
<body>
<main>
<h1>Fake Kick</h1>
<p>Click the made-up user to sign in as.</p>
<form method="get" action="/oauth/authorize">
{hidden}
<div class="users">{users}</div>
<div class="other">
<input name="login" placeholder="Any other login" pattern="[A-Za-z0-9_\\-]{{1,25}}" required>
<button type="submit">Sign in</button>
</div>
</form>
<p>The same login on the fake Twitch is another user until linked. A login ending in 1, 2 or 3 subscribes to everyone. <a href="{cancel}">Cancel</a></p>
</main>
</body>
</html>
"""


def user_id_of(login: str) -> str:
    return str(
        int.from_bytes(hashlib.sha256(f"kick:{login}".encode()).digest()[:8]) % 90_000_000 + 10_000_000
    )


def user_of(login: str) -> dict:
    return {
        "user_id": user_id_of(login),
        "name": login,
        "email": f"{login}@example.com",
        "profile_picture": None,
    }


def channel_of(login: str) -> dict:
    return {
        "broadcaster_user_id": user_id_of(login),
        "slug": login,
        "channel_description": "",
        "stream": {"is_live": False},
    }


def badges_of(login: str) -> list[dict]:
    if login[-1:] in ["1", "2", "3"]:
        return [{"type": "subscriber", "text": "Subscriber", "count": 1, "active": True}]
    return []


def challenge_of(verifier: str) -> str:
    return base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()


class FakeKick:
    def __init__(self, port: int = 0):
        self.signed_in: set[str] = set()
        self.url = ""
        self._port = port
        self._server: ThreadingHTTPServer | None = None
        self._challenges: dict[str, str] = {}
        self._lock = threading.Lock()

    def __enter__(self):
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()

    def start(self):
        self._server = ThreadingHTTPServer(("0.0.0.0", self._port), self._handler())
        self.url = f"http://localhost:{self._server.server_address[1]}"
        LOGGER.info("Fake Kick at %s", self.url)
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
                channel_user = re.match(r"^/api/v2/channels/([^/]+)/users/([^/]+)$", parts.path)
                if parts.path == "/oauth/authorize":
                    self.authorize(query)
                elif parts.path == "/public/v1/users":
                    self.answer(user_of)
                elif parts.path == "/public/v1/channels" and "slug" in query:
                    self.channel_by_slug(query["slug"].lower())
                elif parts.path == "/public/v1/channels":
                    self.answer(channel_of)
                elif channel_user:
                    self.channel_user(channel_user[1].lower(), channel_user[2].lower())
                else:
                    self.json({"message": "Not Found"}, 404)

            def do_POST(self):  # pylint: disable=invalid-name
                length = int(self.headers.get("content-length", "0"))
                form = {key: values[0] for key, values in parse_qs(self.rfile.read(length).decode()).items()}
                if urlsplit(self.path).path != "/oauth/token":
                    self.json({"error": "invalid_request", "error_description": "bad path"}, 400)
                    return
                if form.get("grant_type") == "client_credentials":
                    self.json({"access_token": APP_TOKEN, "token_type": "Bearer", "expires_in": 3600})
                    return
                code = form.get("code", "")
                if not LOGIN_RE.match(code):
                    self.json({"error": "invalid_request", "error_description": "bad code"}, 400)
                    return
                with fake._lock:  # pylint: disable=protected-access
                    challenge = fake._challenges.pop(code, None)  # pylint: disable=protected-access
                if challenge is None or challenge_of(form.get("code_verifier", "")) != challenge:
                    self.json({"error": "invalid_grant", "error_description": "bad code verifier"}, 400)
                    return
                fake.signed_in.add(code)
                self.json(
                    {
                        "access_token": code,
                        "token_type": "Bearer",
                        "expires_in": 7200,
                        "scope": "user:read channel:read",
                    }
                )

            def answer(self, body_of):
                token = self.headers.get("authorization", "").removeprefix("Bearer ")
                if not LOGIN_RE.match(token):
                    self.json({"message": "Unauthorized"}, 401)
                    return
                self.json({"data": [body_of(token)], "message": "OK"})

            def channel_by_slug(self, slug: str):
                if self.headers.get("authorization", "") != f"Bearer {APP_TOKEN}":
                    self.json({"message": "Unauthorized"}, 401)
                    return
                data = [channel_of(slug)] if LOGIN_RE.match(slug) and slug != "nobody" else []
                self.json({"data": data, "message": "OK"})

            def channel_user(self, streamer: str, viewer: str):
                if not LOGIN_RE.match(streamer) or not LOGIN_RE.match(viewer):
                    self.json({"message": "Not Found"}, 404)
                    return
                self.json(
                    {
                        "id": 1,
                        "username": viewer,
                        "slug": viewer,
                        "is_moderator": False,
                        "badges": badges_of(viewer),
                    }
                )

            def authorize(self, query: dict[str, str]):
                redirect_uri = query.get("redirect_uri")
                state = query.get("state", "")
                challenge = query.get("code_challenge", "")
                if redirect_uri is None or query.get("code_challenge_method") != "S256" or not challenge:
                    self.json(
                        {"error": "invalid_request", "error_description": "no redirect_uri or PKCE"}, 400
                    )
                    return
                if "cancel" in query:
                    self.redirect(redirect_uri, {"error": "access_denied", "state": state})
                    return
                login = query.get("login", "").lower()
                if LOGIN_RE.match(login):
                    with fake._lock:  # pylint: disable=protected-access
                        fake._challenges[login] = challenge  # pylint: disable=protected-access
                    self.redirect(redirect_uri, {"code": login, "state": state})
                    return
                hidden = "".join(
                    f'<input type="hidden" name="{html.escape(key)}" value="{html.escape(value)}">'
                    for key, value in query.items()
                    if key not in ["login", "cancel"]
                )
                users = USERS + [
                    (login, "signed in before")
                    for login in sorted(fake.signed_in)
                    if login not in [user for user, _ in USERS]
                ]
                buttons = "".join(
                    f'<button type="submit" formnovalidate name="login" value="{login}"><b>{login}</b><small>{about}</small></button>'
                    for login, about in users
                )
                page = PAGE.format(
                    hidden=hidden,
                    users=buttons,
                    cancel=html.escape(f"/oauth/authorize?{urlencode({**query, 'cancel': '1'})}"),
                )
                self.respond(200, "text/html; charset=utf-8", page.encode())

            def redirect(self, url: str, params: dict[str, str]):
                separator = "&" if "?" in url else "?"
                self.send_response(302)
                self.send_header("location", f"{url}{separator}{urlencode(params)}")
                self.send_header("content-length", "0")
                self.end_headers()

            def json(self, body: dict, status: int = 200):
                self.respond(status, "application/json", json.dumps(body).encode())

            def respond(self, status: int, content_type: str, body: bytes):
                self.send_response(status)
                self.send_header("content-type", content_type)
                self.send_header("content-length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

        return Handler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8082)
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    with FakeKick(args.port):
        threading.Event().wait()


if __name__ == "__main__":
    main()
