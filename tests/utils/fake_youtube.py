"""Pretends to be YouTube's oEmbed endpoint, which the backend looks every requested video up at.

Every video exists, titled after its id, except the ids in unavailable, which can't be embedded. A
test takes YouTube down with down.
"""

import json
import logging
import threading
from http.server import BaseHTTPRequestHandler
from http.server import ThreadingHTTPServer
from urllib.parse import parse_qs
from urllib.parse import urlsplit

LOGGER = logging.getLogger(__name__)
WATCH = "https://www.youtube.com/watch?v="


class FakeYouTube:
    def __init__(self, port: int = 0):
        self.unavailable: set[str] = set()
        self.down = False
        self.lookups: list[str] = []
        self.url = ""
        self._port = port
        self._server: ThreadingHTTPServer | None = None

    def reset(self):
        self.unavailable = set()
        self.down = False
        self.lookups = []

    def start(self):
        self._server = ThreadingHTTPServer(("0.0.0.0", self._port), self._handler())
        self.url = f"http://localhost:{self._server.server_address[1]}"
        LOGGER.debug("Fake YouTube at %s", self.url)
        threading.Thread(target=self._server.serve_forever, args=(0.05,), daemon=True).start()

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
                watched = query.get("url", "")
                if fake.down:
                    self.answer(503, "Service Unavailable")
                elif parts.path != "/oembed" or not watched.startswith(WATCH):
                    self.answer(404, "Not Found")
                else:
                    video = watched.removeprefix(WATCH)
                    fake.lookups.append(video)
                    if video in fake.unavailable:
                        self.answer(401, "Unauthorized")
                    else:
                        self.answer(200, json.dumps({"title": f"Video {video}", "type": "video"}))

            def answer(self, status: int, body: str):
                data = body.encode()
                self.send_response(status)
                self.send_header("Content-Type", "application/json" if status == 200 else "text/plain")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

        return Handler
