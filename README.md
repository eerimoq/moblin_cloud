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
can add songs to a streamer's music queue from `/song-requests/?streamer=<id>`
on the website.

A streamer creates their page at `/song-requests/`, which asks
`POST /remote-control/streamers` for a random secret key and the public id
derived from it (the first 12 bytes of its SHA-256, base64url). They paste
`wss://api.moblin.app/remote-control/streamer/<key>` as the assistant URL in
Moblin under Settings, Remote control, Streamer. Moblin keeps that WebSocket
open and the backend speaks the assistant side of the protocol: it sends
`hello`, accepts any `identify` (the key in the URL is the secret, Moblin's
password is never known to the backend), answers `ping` with `pong`, and
drops streamers silent for 60 seconds.

`GET /remote-control/<id>/music` asks the streamer for its queue, cached for
10 seconds, and `POST /remote-control/<id>/music` with `{"song", "user"}`
requests a song, at most one every 3 seconds per streamer. The song must be
an Apple Music song link (an album link with `?i=<id>`, or a `/song/` link);
anything else answers 400. It is sent to Moblin as
`https://music.apple.com/song/<id>?i=<id>`, since Moblin looks songs up by
`i`. The backend first asks Moblin with a `find` music command whether the
song is on the streamer's Apple Music, answering 422 when it isn't, and then
answers 202 and puts the song in its queue of requested songs. Both answer
404 when the streamer is not connected. Error answers are sentences meant to
be shown to the viewer as they are.

Requested songs are added by a background task, which retries until the song
is in the streamer's queue: whenever the streamer connects, right after a
request and every 10 seconds. Every add carries a request id as `requestId`
(random for free requests, the PayPal order id for paid ones). Moblin keeps
it on the song in its playlist and answers `ok` without adding again when a
song in the playlist already has it, so resending an add whose answer got lost
is safe (unless the streamer removed the song in between). It answers `error` when it could not add the song, and the task retries.
The task drops a song Moblin can't find and gives up after 7 days. The songs not yet added are saved in the JSON file
given by `--song-queue-file` (or `SONG_QUEUE_FILE`), which needs a volume in
Docker; without one they are forgotten on restart.

Streamers manage their page at `/song-requests/`, which shows the viewer page
only when given a name or `?streamer=<id>`. The secret key authenticates them: `GET /remote-control/streamer/<key>/settings` answers
`{"id", "connected", "name"}` and `POST` with `{"name"}` changes them. A
name is 3 to 25 lowercase letters, digits and dashes, unique, and gives the page a second address, `/song-requests/<name>`. GitHub
Pages has no such file, so it serves `404.html`, which renders the song request
page for those paths and resolves the name with
`GET /remote-control/names/<name>`, answering `{"id"}`. The dev server
rewrites the same paths to `404.html`. Settings are saved in the JSON file
given by `--song-requests-file` (or `SONG_REQUESTS_FILE`), which needs a
volume when running in Docker; without one they are forgotten on restart.

### Paid song requests

A streamer can ask viewers to pay for every song, with PayPal. They enter
their PayPal email, a price of 0.50 to 1000 and a currency on their page,
which the settings endpoint takes as `"paywall": {"email", "price",
"currency"}` (`price` in hundredths) or `null`. Saving looks up the merchant
ID of the email with a PayPal order that is never paid, and answers 422 when
PayPal can't send payments to it. The money goes straight to the streamer:
every PayPal order names the streamer as payee, so the backend never holds
any of it and PayPal takes its fees from the streamer.

While a paywall is set `POST /remote-control/<id>/music` answers 402.
`GET /remote-control/<id>/paywall` answers `{"paywall": {"price", "currency",
"clientId", "merchantId"}}` or `{"paywall": null}`, without the email. The
viewer page loads the PayPal JavaScript SDK with those and shows PayPal's
buttons, which open PayPal's checkout in a popup over the page. The buttons
create the order with `POST /remote-control/<id>/music/orders` and `{"song",
"user"}`, which checks the song like a free request does, so nobody pays for a
song that can't be added. It answers `{"orderId"}` and remembers the song for
three hours. Once the viewer approves, the page posts to
`/remote-control/<id>/music/orders/<order id>/capture`. The backend captures
the payment only while the streamer is connected, checks the amount, and then
queues the song like a free one, answering 202.

The backend needs the client ID and secret of a
[PayPal REST app](https://developer.paypal.com/dashboard/applications), given
with `--paypal-client-id` and `--paypal-client-secret` (or `PAYPAL_CLIENT_ID`
and `PAYPAL_CLIENT_SECRET`). `--paypal-sandbox` (or `PAYPAL_SANDBOX=true`)
uses the PayPal sandbox instead. Without credentials streamers can't turn
paid requests on, and paywalled pages answer 503 when a viewer tries to pay.

`just backend-fake-streamer [key]` connects a made-up streamer to a local
backend, for developing the page.
