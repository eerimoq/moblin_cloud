# Moblin website

Landing page for Moblin and the rest of the Moblin family: Mobcam, Moblink and
the remote control relays.

The website lives in `frontend/` and its backend in `backend/`.

## Develop

```
cd frontend
npm install
npm run dev
```

The "Streamers" section is fetched from the backend in `backend/`, a small
Rust service the deployed site expects at `https://api.moblin.app`. The dev
server proxies `/api` to one running locally, so start it in another terminal:

```
just backend-run
```

## Printing the trading cards

```
just cards-print
```

builds the site and writes every card and the shared back to `cards-print/` as
822x1122 PNGs at 300 dpi: a 2.5x3.5 inch poker card with 36 px bleed on each
side, the size makeplayingcards.com asks for. Card text stays 36 px inside the
cut line. It needs Google Chrome installed and warns about any card whose text
overflows or leaves that safe area.

## Backend

The Moblin app tells the backend who went live by posting to
`/streamers/live`. Only the Moblin app itself may do so: every post is signed
with [App Attest](https://developer.apple.com/documentation/devicecheck/establishing-your-app-s-integrity),
and the backend verifies the attestation and assertion before listing anyone
(see `backend/src/app_attest.rs`). Posts must carry a fresh one-time
challenge from `/streamers/live/challenge`.

The App ID and App Attest environment the backend accepts default to the
distributed Moblin app, and can be changed with `--app-id` and
`--app-attest-environment` (apps installed by Xcode use the `development`
environment). `just backend-run-attested` accepts the development environment
and listens on all interfaces, so a phone on the same network can post live
from a Moblin build installed by Xcode. `just backend-run` starts the backend with
`--allow-unattested` so that `scripts/seed_backend.py` can fill it with
made-up streamers.

Twitch display names and avatars come from the
[Helix users endpoint](https://dev.twitch.tv/docs/api/reference/#get-users),
authenticated with an app access token from the
[client credentials grant flow](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/#client-credentials-grant-flow).
Register an application in the
[Twitch developer console](https://dev.twitch.tv/console/apps) and pass its
client ID and secret with `--twitch-client-id` and `--twitch-client-secret`
(or `TWITCH_CLIENT_ID` and `TWITCH_CLIENT_SECRET`). Without them Twitch
streamers are listed without a display name and avatar.

The same credentials let the backend tell who is live on Twitch, from the
[Helix streams endpoint](https://dev.twitch.tv/docs/api/reference/#get-streams).
Every channel in `/streamers` carries a `live` flag, refreshed every five
minutes, and one minute after anyone posts that they went live. All tracked
Twitch channels are checked in a single request; Kick channels one by one
through `kick.com/api/v2/channels`, which needs no credentials. YouTube
channels are never live. `/streamers` lists live streamers first and the
most recently posted first within each group, at most `--max-streamers` of
them. The backend always remembers the most recent posters, so that a
newcomer is around when the live check runs, and on top of those any older
streamer who is still live, up to another `--max-streamers`. The "Erik is live on Twitch"
button in the Support section asks `/twitch/live` instead, which answers
`{"channel": "eerimoq", "live": true}`, asking Twitch at most once a minute
no matter how many visitors ask. The channel can be changed with
`--twitch-live-channel`. Without Twitch credentials nobody is ever live, the
endpoint answers 503 and the website shows the button as not live.

## Build

```
cd frontend
npm run build
```

## Deploy

Every push to `main` builds the site and deploys it to GitHub Pages through
`.github/workflows/deploy.yml`. Enable Pages in the repository settings with
"GitHub Actions" as the source.

Without a custom domain the site is served from
https://eerimoq.github.io/moblin_website/ and the build uses `/moblin_website/`
as its base path. To serve it on a custom domain, add a `frontend/public/CNAME` file
containing the domain and point the domain's DNS at GitHub Pages. When that
file exists the build switches to `/` automatically (see
`frontend/vite.config.ts`).

## Song requests

The backend also acts as a Moblin remote control assistant, so that viewers
can add songs to a streamer's music queue from `/song-request/?streamer=<id>`
on the website.

A streamer creates their page at `/song-request/`, which asks
`POST /remote-control/streamers` for a random secret key and the public id
derived from it (the first 12 bytes of its SHA-256, base64url). They paste
`wss://api.moblin.app/remote-control/streamer/<key>` as the assistant URL in
Moblin under Settings, Remote control, Streamer. Moblin keeps that WebSocket
open and the backend speaks the assistant side of the protocol: it sends
`hello`, accepts any `identify` (the key in the URL is the secret, Moblin's
password is never known to the backend), answers `ping` with `pong`, and
drops streamers silent for 60 seconds.

`GET /remote-control/<id>/music` asks the streamer for its queue, cached for
10 seconds, and `POST /remote-control/<id>/music` with `{"song", "user"}` adds
a song, at most one every 3 seconds per streamer. A song ID or an Apple Music
song link is turned into a link with an `i=<id>` query parameter, which is
what Moblin looks songs up by; anything else is searched for. Both answer 404
when the streamer is not connected.

Streamers manage their page at `/song-request/admin/`. The secret key
authenticates them: `GET /remote-control/streamer/<key>/settings` answers
`{"id", "connected", "enabled", "name"}` and `POST` with `{"enabled",
"name"}` changes them. While song requests are off both music endpoints answer
403. A name is 3 to 25 lowercase letters, digits and dashes, unique and not
`admin`, and gives the page a second address, `/song-request/<name>`. GitHub
Pages has no such file, so it serves `404.html`, which renders the song request
page for those paths and resolves the name with
`GET /remote-control/names/<name>`, answering `{"id"}`. The dev server
rewrites the same paths to `404.html`. Settings are saved in the JSON file
given by `--song-requests-file` (or `SONG_REQUESTS_FILE`), which needs a
volume when running in Docker; without one they are forgotten on restart.

`just backend-fake-streamer [key]` connects a made-up streamer to a local
backend, for developing the page.
