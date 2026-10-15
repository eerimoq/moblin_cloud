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
can add songs to a streamer's music queue from `/twitch/<login>` (or
`/song-requests/?streamer=<id>`) on the website.

A streamer signs in with Twitch at `/song-requests/` (see Signing in) and gets
their page right away, with a random secret key and a public id (the first
12 bytes of the key's SHA-256, base64url). The id stays when the streamer gets
a new key, so it is stored with the account rather than derived again. They
paste
`wss://api.moblin.app/remote-control/streamer/<key>` as the assistant URL in
Moblin under Settings, Remote control, Streamer. Moblin keeps that WebSocket
open and the backend speaks the assistant side of the protocol: it sends
`hello`, accepts any `identify` (the key in the URL is the secret, Moblin's
password is never known to the backend), answers `ping` with `pong`, and
drops streamers silent for 60 seconds. Only the current key of an account's
page is accepted.

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
only when given `?streamer=<id>`. The settings of a page are
`{"id", "connected", "paywall", "paypal", "title", "message", "accent"}`, read
and changed through the account endpoints below. `title` (at most 60
characters) and `message` (at most 300) are shown on the viewer page, and
`accent` is one of `green`, `purple`, `blue`, `pink`, `orange` and `red`; all
three are `null` when unset. `GET /remote-control/<id>/profile` answers
`{"name", "login", "avatar", "title", "message", "accent"}` for the viewer
page. Settings are saved in the JSON file
given by `--song-requests-file` (or `SONG_REQUESTS_FILE`), which needs a
volume when running in Docker; without one they are forgotten on restart.

### Signing in

Streamers must sign in with Twitch to create or manage a page, which is kept
with their account, is at `/twitch/<login>` and can be managed from any
browser. Pages made before signing in was required can't be added to an
account; those streamers get a new page and streamer URL. The sign in button
links to
`GET /auth/twitch/login`, which redirects to Twitch with a one-time `state`
that lives 10 minutes. Twitch redirects back to `GET /auth/twitch/callback`,
which exchanges the code, looks the user up and redirects to
`<frontend url>/song-requests/#session=<session>` (or `#signInFailed`). A
session is the Twitch user id and an expiry 30 days later, signed with
HMAC-SHA256, and is sent as `Authorization: Bearer <session>`. Signing out only
forgets it in the browser.

`GET /account` answers `{"name", "login", "streamerUrl", "settings"}`, where
`streamerUrl` and `settings` are `null` until the account has a page. The key
is only ever handed out inside the streamer URL, which the backend builds from
`--public-url` (`https` becomes `wss`). The admin page then
creates one at once with `POST /account/streamer`, which answers with the
existing page when the account already has one, so two tabs can't replace each
other's page. `POST /account/streamer-url` resets the streamer URL with a
new key, for when it leaked: the old one stops working at once and a Moblin
connected with it is disconnected, while the page, its settings and queued
songs stay. `POST /account/settings` with `{"paywall", "title", "message", "accent"}`
changes the settings.

GitHub Pages has no file for `/twitch/<login>`, so it serves `404.html`,
which renders the song request page for those paths (the dev server rewrites
them to `404.html` too) and resolves the login with `GET
/remote-control/names/twitch/<login>`, answering `{"id"}`. The `twitch`
prefix leaves room for other platforms later. `GET
/remote-control/<id>/profile` answers `{"name", "login", "avatar"}`, the
Twitch display name, login and profile image saved at the streamer's last sign
in (or 404), which the song request page shows at the top so viewers know
whose page it is. Logins match
case-insensitively. Each sign in records the login, and takes it from any other
account that still has it after a rename. Accounts are saved in the JSON file
given by `--accounts-file` (or `ACCOUNTS_FILE`), which needs a volume in
Docker; without one they are forgotten on restart.

Sign in uses the same Twitch application as the profile lookups; register
`https://api.moblin.app/auth/twitch/callback` as one of its redirect URLs.
Without Twitch credentials `GET /auth/twitch/login` answers 503.
`--session-secret` (or `SESSION_SECRET`) signs the sessions; without it a
random secret is used and everyone is signed out on restart. `--public-url`
(default `https://api.moblin.app`) is where the backend is reached, for the
redirect URL, and `--frontend-url` (default `https://moblin.app`) where
streamers are sent after signing in. `just backend-run` passes `--public-url
http://localhost:5173/api --frontend-url http://localhost:5173`, so that sign
in goes through the dev server's proxy, and `--fake-twitch-sign-in`, which
never asks Twitch: signing in goes straight to the callback as the made-up
Twitch user `streamer` (account id `fake-streamer`), or as another login with
`/api/auth/twitch/login?login=<login>`. Never use it in production.

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
"clientId", "merchantId"}}` or `{"paywall": null}`, without the email.
Saving a paywall fails unless PayPal tells the merchant ID, and a paywall
without one answers `null` for both IDs and can't be paid. The
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

To try song requests locally without Twitch or a phone:

1. `just backend-run` and `just frontend-run`.
2. Open `http://localhost:5173/song-requests/`, sign in with Twitch (faked)
   and your page is created.
3. Copy the streamer URL from the page and run
   `just backend-fake-streamer <streamer URL>`, which connects a
   made-up Moblin to the backend. Without a key it makes up a new one, only
   reachable at `/song-requests/?streamer=<id>` and not manageable.
4. Request songs at `http://localhost:5173/twitch/streamer`. Songs with the id
   404 are not found.
