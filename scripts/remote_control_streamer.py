#!/usr/bin/env python3

import argparse
import asyncio
import json
import urllib.request

import websockets

BACKEND = "localhost:8080"


def create_streamer():
    request = urllib.request.Request(f"http://{BACKEND}/remote-control/streamers", method="POST")
    with urllib.request.urlopen(request) as response:
        return json.load(response)


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
        if "add" in command:
            title = command["add"]["title"]
            if "notfound" in title:
                return self.status(song_not_found=True)
            self.next_id += 1
            song = {"id": str(self.next_id), "title": title, "artist": "Fake Artist"}
            if "user" in command["add"]:
                song["user"] = command["add"]["user"]
            self.songs.append(song)
            if self.current_song_index is None:
                self.current_song_index = 0
            return self.status(song_not_found=False)
        return self.status()


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
                            "data": {
                                "music": {
                                    "status": player.handle(request["data"]["music"]["command"])
                                }
                            },
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
    parser.add_argument("key", nargs="?", help="Streamer key. A new one is created if not given.")
    args = parser.parse_args()
    key = args.key
    if key is None:
        streamer = create_streamer()
        key = streamer["key"]
        print(f"Key: {key}")
        print(f"Page: http://localhost:5173/song-request/?streamer={streamer['id']}")
    asyncio.run(run(key))


if __name__ == "__main__":
    main()
