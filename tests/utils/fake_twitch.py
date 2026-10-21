#!/usr/bin/env python3
"""Pretends to be Twitch's sign in (OAuth) and the Helix endpoints it needs, for signing in as made-up users.

Any login signs in. Its Twitch user id is fake-<login>. A login ending in 1, 2 or 3 is a subscriber
of that tier to everyone, and any login (except nobody) resolves to a Twitch user, so a streamer can
add it as a moderator.
"""

import argparse
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
LOGIN_RE = re.compile(r"^[a-z0-9_]{1,25}$")
APP_TOKEN = "app-token"
USERS = [
    ("admin", "an admin, and so premium"),
    ("premium", "a premium account"),
    ("normal", "neither admin nor premium"),
    ("sub1", "a tier 1 subscriber"),
    ("sub2", "a tier 2 subscriber"),
    ("sub3", "a tier 3 subscriber"),
]

PAGE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Fake Twitch</title>
<style>
body {{ font-family: system-ui, sans-serif; background: #0e0e10; color: #efeff1; display: grid; place-items: center; min-height: 100vh; margin: 0; }}
main {{ background: #18181b; padding: 32px; border-radius: 12px; width: min(420px, calc(100vw - 32px)); box-sizing: border-box; }}
h1 {{ color: #9147ff; margin-top: 0; }}
form {{ margin: 0; }}
.users {{ display: flex; flex-direction: column; gap: 6px; }}
.users button {{ display: flex; justify-content: space-between; gap: 16px; width: 100%; text-align: left; font: inherit; padding: 12px 16px; border-radius: 8px; border: 0; background: #26262c; color: inherit; cursor: pointer; }}
.users button:hover {{ background: #9147ff; }}
.users button:hover small {{ color: #efeff1; }}
.users button b {{ font-weight: 600; }}
.users small {{ color: #adadb8; }}
.other {{ display: flex; gap: 8px; margin-top: 20px; }}
input {{ flex: 1; min-width: 0; font: inherit; padding: 10px; border-radius: 6px; border: 1px solid #3a3a3d; background: #0e0e10; color: inherit; }}
.other button {{ font: inherit; padding: 10px 16px; border-radius: 6px; border: 0; background: #3a3a3d; color: white; cursor: pointer; }}
p {{ color: #adadb8; font-size: 14px; }}
a {{ color: #adadb8; }}
</style>
</head>
<body>
<main>
<h1>Fake Twitch</h1>
<p>Click the made-up user to sign in as.</p>
<form method="get" action="/oauth2/authorize">
{hidden}
<div class="users">{users}</div>
<div class="other">
<input name="login" placeholder="Any other login" pattern="[A-Za-z0-9_]{{1,25}}" required>
<button type="submit">Sign in</button>
</div>
</form>
<p>A login ending in 1, 2 or 3 subscribes at that tier. <a href="{cancel}">Cancel</a></p>
</main>
</body>
</html>
"""


def user_of(login: str) -> dict:
    return {"id": f"fake-{login}", "login": login, "display_name": login, "profile_image_url": None}


def tier_of(login: str) -> str | None:
    return {"1": "1000", "2": "2000", "3": "3000"}.get(login[-1])


class FakeTwitch:
    def __init__(self, port: int = 0):
        self.signed_in: set[str] = set()
        self.url = ""
        self._port = port
        self._server: ThreadingHTTPServer | None = None

    def __enter__(self):
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()

    def start(self):
        self._server = ThreadingHTTPServer(("0.0.0.0", self._port), self._handler())
        self.url = f"http://localhost:{self._server.server_address[1]}"
        LOGGER.info("Fake Twitch at %s", self.url)
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
                if parts.path == "/oauth2/authorize":
                    self.authorize(query)
                elif parts.path == "/helix/users" and "login" in query:
                    login = query["login"].lower()
                    if self.headers.get("authorization") != f"Bearer {APP_TOKEN}":
                        self.json({"error": "Unauthorized", "status": 401, "message": "bad token"}, 401)
                    elif LOGIN_RE.match(login) and login != "nobody":
                        self.json({"data": [user_of(login)]})
                    else:
                        self.json({"data": []})
                elif parts.path == "/helix/users":
                    self.json({"data": [user_of(login) for login in [self.login()] if login]})
                elif parts.path == "/helix/subscriptions/user":
                    tier = tier_of(query.get("user_id", "").removeprefix("fake-"))
                    if tier is None:
                        self.json({"error": "Not Found", "status": 404, "message": "not subscribed"}, 404)
                    else:
                        self.json(
                            {
                                "data": [
                                    {
                                        "broadcaster_id": query.get("broadcaster_id"),
                                        "tier": tier,
                                        "is_gift": False,
                                    }
                                ]
                            }
                        )
                else:
                    self.json({"error": "Not Found", "status": 404}, 404)

            def do_POST(self):  # pylint: disable=invalid-name
                length = int(self.headers.get("content-length", "0"))
                form = {key: values[0] for key, values in parse_qs(self.rfile.read(length).decode()).items()}
                if urlsplit(self.path).path != "/oauth2/token":
                    self.json({"error": "Bad Request", "status": 400, "message": "bad path"}, 400)
                    return
                if form.get("grant_type") == "client_credentials":
                    self.json({"access_token": APP_TOKEN, "token_type": "bearer", "expires_in": 3600})
                    return
                if not LOGIN_RE.match(form.get("code", "")):
                    self.json({"error": "Bad Request", "status": 400, "message": "bad code"}, 400)
                    return
                login = form["code"]
                fake.signed_in.add(login)
                self.json({"access_token": login, "token_type": "bearer", "expires_in": 3600, "scope": []})

            def login(self) -> str | None:
                token = self.headers.get("authorization", "").removeprefix("Bearer ")
                return token if LOGIN_RE.match(token) else None

            def authorize(self, query: dict[str, str]):
                redirect_uri = query.get("redirect_uri")
                state = query.get("state", "")
                if redirect_uri is None:
                    self.json({"error": "Bad Request", "status": 400, "message": "no redirect_uri"}, 400)
                    return
                if "cancel" in query:
                    self.redirect(redirect_uri, {"error": "access_denied", "state": state})
                    return
                login = query.get("login", "").lower()
                if LOGIN_RE.match(login):
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
                    cancel=html.escape(f"/oauth2/authorize?{urlencode({**query, 'cancel': '1'})}"),
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
    parser.add_argument("--port", type=int, default=8081)
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    with FakeTwitch(args.port):
        threading.Event().wait()


if __name__ == "__main__":
    main()
