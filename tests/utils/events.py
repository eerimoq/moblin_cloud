import json
import queue
import socket
import threading
import time

from .frontend import BackendError
from .frontend import Client


class EventStream:
    """A server-sent event stream read in the background like the frontend's sse.ts, so a test can wait
    for an event with a timeout or check that none came. Events without data are kept too, with None."""

    def __init__(self, client: Client, path: str):
        self._response = client.request("GET", path, headers={"accept": "text/event-stream"}, stream=True)
        if not self._response.ok:
            text = self._response.text
            self._response.close()
            raise BackendError(self._response.status_code, text)
        self._events: queue.Queue[tuple[str, object] | None] = queue.Queue()
        self.ended = threading.Event()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        name: str | None = None
        data: list[str] = []
        try:
            for raw in self._response.iter_lines(decode_unicode=True):
                line = raw if isinstance(raw, str) else raw.decode()
                if line.startswith(":"):
                    continue
                if line.startswith("event:"):
                    name = line[6:].strip()
                elif line.startswith("data:"):
                    data.append(line[5:].removeprefix(" "))
                elif line == "":
                    if name is not None or data:
                        text = "\n".join(data)
                        try:
                            value = json.loads(text) if text else None
                        except ValueError:
                            value = text
                        self._events.put((name or "message", value))
                    name = None
                    data = []
        except Exception:  # pylint: disable=broad-exception-caught
            pass
        finally:
            self.ended.set()
            self._events.put(None)

    def next(self, timeout: float = 10) -> tuple[str, object]:
        try:
            event = self._events.get(timeout=timeout)
        except queue.Empty as error:
            raise TimeoutError("No event in time.") from error
        if event is None:
            self._events.put(None)
            raise EOFError("The event stream ended.")
        return event

    def wait_for(self, name: str, timeout: float = 10) -> object:
        deadline = time.monotonic() + timeout
        while True:
            left = deadline - time.monotonic()
            if left <= 0:
                raise TimeoutError(f"No {name} event in time.")
            event, data = self.next(left)
            if event == name:
                return data

    def drain(self, quiet: float = 0.5) -> list[str]:
        """Reads the events until none came for a while, returning their names."""
        names = []
        while True:
            try:
                names.append(self.next(quiet)[0])
            except (TimeoutError, EOFError):
                return names

    def wait_for_end(self, timeout: float) -> bool:
        return self.ended.wait(timeout)

    def close(self):
        """Closes the connection at once, even while the reader waits for the next keep alive."""
        try:
            self._response.raw._fp.fp.raw._sock.shutdown(socket.SHUT_RDWR)  # pylint: disable=protected-access
        except (AttributeError, OSError):
            pass
        self._response.close()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()
