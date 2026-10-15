#!/usr/bin/env python3

import argparse
import asyncio
import base64
import hashlib
import json
import secrets

import websockets

BACKEND = "localhost:8080"


def base64url(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def id_of(key):
    return base64url(hashlib.sha256(key.encode()).digest()[:12])


def found(link):
    return not link.endswith("i=404")


class Player:
    def __init__(self):
        self.next_id = 0
        self.songs = []
        self.current_song_index = None

    def status(self, song_not_found=None):
        status = {
            "playing": self.current_song_index is not None,
            "songs": self.songs,
        }
        if self.current_song_index is not None:
            status["currentSongIndex"] = self.current_song_index
        if song_not_found is not None:
            status["songNotFound"] = song_not_found
        return status

    def handle(self, command):
        if "find" in command:
            if found(command["find"]["title"]):
                return {"musicFind": {"song": {"id": "1", "title": "Fake Song", "artist": "Fake Artist"}}}
            return {"musicFind": {}}
        if "add" in command:
            add = command["add"]
            if "requestId" in add and any(song.get("requestId") == add["requestId"] for song in self.songs):
                return {"music": {"status": self.status(song_not_found=False)}}
            if not found(add["title"]):
                return {"music": {"status": self.status(song_not_found=True)}}
            self.next_id += 1
            song = {"id": str(self.next_id), "title": add["title"], "artist": "Fake Artist"}
            for name in ["user", "requestId"]:
                if name in add:
                    song[name] = add[name]
            self.songs.append(song)
            if self.current_song_index is None:
                self.current_song_index = 0
            return {"music": {"status": self.status(song_not_found=False)}}
        return {"music": {"status": self.status()}}


async def ping(websocket):
    while True:
        await asyncio.sleep(30)
        await websocket.send(json.dumps({"ping": {}}))


async def run(key):
    player = Player()
    async with websockets.connect(f"ws://{BACKEND}/remote-control/streamer/{key}") as websocket:
        pinger = asyncio.create_task(ping(websocket))
        try:
            async for text in websocket:
                message = json.loads(text)
                print("<-", text)
                if "hello" in message:
                    reply = {"identify": {"streamerId": "FAKE", "authentication": "fake"}}
                elif "request" in message:
                    request = message["request"]
                    reply = {
                        "response": {
                            "id": request["id"],
                            "result": {"ok": {}},
                            "data": player.handle(request["data"]["music"]["command"]),
                        }
                    }
                else:
                    continue
                print("->", json.dumps(reply))
                await websocket.send(json.dumps(reply))
        finally:
            pinger.cancel()


def main():
    parser = argparse.ArgumentParser(
        description="Pretends to be a Moblin streamer connected to the backend over remote control."
    )
    parser.add_argument(
        "url", nargs="?", help="Streamer URL. A new one is made up if not given."
    )
    args = parser.parse_args()
    key = (args.url or base64url(secrets.token_bytes(32))).rstrip("/").split("/")[-1]
    print(f"Streamer URL: ws://{BACKEND}/remote-control/streamer/{key}")
    print(f"Page: http://localhost:5173/song-requests/?streamer={id_of(key)}")
    print("Songs with the id 404 are not found.")
    asyncio.run(run(key))


if __name__ == "__main__":
    main()
