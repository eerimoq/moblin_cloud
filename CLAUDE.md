# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

The landing page for Moblin (moblin.app) and its companion apps. Two independent
projects live here:

- `frontend/` — a static site (SolidJS + TypeScript + Tailwind v4 + Vite), deployed to GitHub Pages.
- `backend/` — a small Rust (axum) service deployed at `https://api.moblin.app` as a Docker image. It serves the "Streamers" list and the "Erik is live on Twitch" status.

`README.md` only covers getting started; the deploy flow, App Attest and the Twitch and PayPal setup are described below and in the code.

## Commands

All checks are driven by the `justfile` at the repo root (CI runs exactly these):

```
just style          # oxfmt (frontend) + cargo fmt (backend)
just style-check
just lint           # oxlint --deny-warnings + cargo clippy --all-targets -D warnings
just spell-check    # codespell, configured in .codespellrc
just test           # cargo test (backend only; the frontend has no tests)
```

Run a single backend test: `cd backend && cargo test store::tests::list_is_capped` (tests are inline `#[cfg(test)]` modules per file).

Development needs two terminals:

```
just backend-run    # cargo run --allow-unattested, then seeds fake streamers via scripts/seed_backend.py
just frontend-run   # vite dev server; proxies /api -> localhost:8080
just backend-seed-forever [interval]   # keep posting random streamers to test live updates
```

Frontend build: `cd frontend && npm run build` (runs `tsc --noEmit` first, so type errors fail the build). Backend Docker: `just backend-docker-run`.

## Frontend architecture

- **Multi-page, no router.** Each page is its own HTML entry listed in `vite.config.ts` (`index.html`, `chat-bot/index.html`, `dashboard/index.html`, ...) with its own `src/*.tsx` entry that renders a page component. Sub pages live in a directory so GitHub Pages serves clean URLs like `/chat-bot/`. Add a page by adding all three: the HTML file, the entry, and the `rollupOptions.input` line. `404.html` is what GitHub Pages serves for unknown paths; it renders the page for `/twitch/<login>` (a signed in streamer's page; the prefix leaves room for Kick later) and a not found page otherwise. Streamers sign in and manage their page at `/dashboard/`; `/page/?streamer=<id>` shows a page by id and redirects to the dashboard without one. `song-requests/index.html` only redirects old `/song-requests/` links to those.
- **Base path is dynamic.** `vite.config.ts` uses `/` when `public/CNAME` exists and `/moblin_website/` otherwise. Anything referencing a file under `public/` must go through `asset()` in `src/asset.ts`; never hardcode a leading slash in a src attribute.
- **Content lives in `src/data/`, not in components.** URLs in `links.ts`, products/features/reviews/chat-bot commands in their own files. Components in `src/components/` are one section each and are composed in order in `App.tsx`.
- **Backend access** goes through `data/backend.ts`: `/api` in dev (proxied), `links.api` in production. `data/streamers.ts` fetches the list once as a module-level SolidJS resource; `twitch.ts` polls `/twitch/live` every five minutes and falls back to "not live" on any error.
- **Theming** is Tailwind v4 `@theme` tokens in `src/index.css` (e.g. `bg-card`, `text-leaf`, `font-display`). Dark is the default; light is the same token names overridden under `prefers-color-scheme: light`. Use the tokens, not raw colors.
- Icons are inline stroke SVGs in `components/Icon.tsx` keyed by `IconName`; platform logos on a colored disc are `BrandIcon`.

## Backend architecture

One binary, `backend/src/main.rs`, wired together from:

- `api.rs` — the axum router. Routes: `GET /streamers`, `POST /streamers/live/challenge`, `POST /streamers/live`, `GET /twitch/live`, `GET /remote-control/streamer/{key}` (WebSocket), `GET /remote-control/names/twitch/{login}`, `GET /remote-control/{id}/paywall`, `GET /remote-control/{id}/profile`, `GET`/`POST /remote-control/{id}/music`, `POST /remote-control/{id}/music/orders`, `POST /remote-control/{id}/music/orders/{order_id}/capture`, `GET /remote-control/{id}/tips`, `POST /remote-control/{id}/tips/orders`, `POST /remote-control/{id}/tips/orders/{order_id}/capture`, `GET /remote-control/{id}/spin`, `POST /remote-control/{id}/spin/orders`, `POST /remote-control/{id}/spin/orders/{order_id}/capture`, `GET /auth/twitch/login`, `GET /auth/twitch/callback`, `GET /account`, `POST /account/streamer`, `POST /account/streamer-url`, `POST /account/settings`, `POST /account/paypal`, `POST /account/paypal/check`, `POST /account/sign-out`, `POST /paypal/webhooks`. Pages are only created (automatically, on the first visit after signing in) and managed by signed in Twitch accounts; the secret key is only used by Moblin's WebSocket and never leaves the backend except inside the streamer URL (`Auth::streamer_url`), and the page id is no longer derived from it after creation. `POST /account/streamer` returns the existing page when the account has one. Free requests and captures share the same checks (`song_and_user`, then `requested_song`, which asks Moblin to `find` the song) and both end in `queue`, answering 202. Tips and wheel spins need no connected streamer; a completed capture posts the tip or spin as JSON (`webhook_body`) to the streamer's webhook URL (`webhooks.rs`), optional for tips and required for spins. Song, tip and spin orders share `capture`, which refuses an order of another kind (`Item::kind`). Viewer-facing errors are full sentences the page shows as they are. CORS allows only `https://moblin.app` (hardcoded).
- `store.rs` — in-memory list of streamers, newest post first. It keeps the `--max-streamers` most recent posters (so a newcomer is still around when the live check runs a minute later) plus any older streamer who is still live, up to another `--max-streamers`; `streamers()` lists live streamers first, capped at `--max-streamers`. A streamer is identified by sharing any channel (platform + case-insensitive name); going live again moves to the top and keeps any already-looked-up profile and the live flag. Each listed channel carries a `Lookup` (pending with retry backoff, or done) that serializes flat into the channel JSON as `avatar`/`displayName`, plus `live` (only ever true for Twitch and Kick channels).
- `profiles.rs` — a background task that pulls the next due lookup from the store and resolves display name and avatar per platform (Twitch via Helix, YouTube via `og:` meta tags with unavatar.io fallback, Kick via `kick.rs`, its public channels API), with `--lookup-spacing` between requests.
- `app_attest.rs` — verifies Apple App Attest attestations and assertions so only the Moblin app can post live. Stateless by design: the app resends its attestation every time and the server re-verifies the chain each time, deliberately ignoring certificate validity dates. Tests use public sample vectors in `src/testdata/`.
- `challenges.rs` — one-time 32-byte challenges, 120 s lifetime, that every live post must consume.
- `remote_control.rs` — the assistant side of Moblin's remote control protocol, for song requests on the page. Streamers connect with a secret key in the URL; the public page id is derived from it. Serde types mirror Swift's synthesized `Codable` (`{"ping":{}}`, optional fields omitted), and must match `Moblin/RemoteControl/RemoteControl.swift` in the Moblin repo. Moblin pushes its wheel (the first enabled wheel of luck widget in the current scene) as a `wheelOfLuck` event on connect and on every change; it is kept with the session and served by `GET /remote-control/{id}/spin` as `wheel` for the page to draw. Only `music` requests are sent: `status`, `find` (is a song on the streamer's Apple Music, asked before payment) and `add` (with an optional `requestId`, kept on the song in Moblin's playlist; Moblin answers `ok` without adding again when a playlist song already has it, and `error` when it could not add it). Song requests must be Apple Music song links, sent as `https://music.apple.com/song/<id>?i=<id>`.
- `pages.rs` — per streamer page settings (`Pages`), keyed by the public id: an optional PayPal paywall (price in hundredths, currency), optional tips (currency, one to four suggested amounts in hundredths, optional webhook URL), an optional spin wheel (one to four options of a number of spins and its price, currency, webhook URL) and the look of the page (headline up to 60 characters, message up to 300, accent color from a fixed list whose CSS lives in `index.css` as `[data-accent]` overrides of the moss/leaf tokens). `GET /remote-control/{id}/profile` returns the look with the name and avatar. Saved to `--pages-file` (JSON, written atomically) on every change, in memory only without it.
- `auth.rs` — streamer sign in with Twitch only (OAuth authorization code), with one-time states that last 10 minutes and must match the `moblin_sign_in_state` cookie set by `GET /auth/twitch/login`, so a callback URL only works in the browser that started the sign in. Sessions are `<twitch user id>:<generation>:<issued>` signed with HMAC-SHA256 under `--session-secret` (random without it), 30 days, sent as a bearer token. A session is only accepted while its generation matches the account's `sessions` counter, which signing out increments; starting to connect PayPal (`POST /account/paypal`) needs a session issued in the last 10 minutes. The Twitch URLs are fields of `OAuthApp` so tests can point them at a local server.
- `accounts.rs` — accounts keyed by Twitch user id, each with a display name and Twitch profile image (shown on the page), the login that gives the page its `/twitch/<login>` address (taken from other accounts on sign in, after renames), its page: a public id fixed at creation plus the current secret key, which `POST /account/streamer-url` replaces (the WebSocket route only accepts current keys, via `page_of_key`), and its PayPal: the merchant ID every paid feature of the page pays (`merchant_of`) and the tracking ID of an unfinished onboarding. The merchant ID only ever comes from PayPal's seller onboarding (Partner Referrals): `POST /account/paypal` creates a referral and answers PayPal's sign up link, and `POST /account/paypal/check` looks the tracking ID up with PayPal and connects the merchant ID once it can receive payments, has a confirmed email and granted our app third-party permissions; the return URL's parameters are never trusted. `POST /paypal/webhooks` (verified with PayPal's verify webhook signature API under `--paypal-webhook-id`) connects pending onboardings on `MERCHANT.ONBOARDING.COMPLETED` through the same `finish_onboarding` checks and disconnects merchants on `MERCHANT.PARTNER-CONSENT.REVOKED`. Paid features can only be turned on or changed while connected. Saved to `--accounts-file` like the song request settings.
- `paypal.rs` — PayPal partner client: seller onboarding (`referral`, `seller`, which need the partner's merchant ID `--paypal-partner-id`; `--paypal-attribution-id` is the BN code sent with every request), webhook verification and Orders v2, created and captured for the streamer with an unsigned `PayPal-Auth-Assertion` JWT naming their merchant ID, for paid song requests, tips and wheel spins (an order's `Item` is a song, a tip with its message or a spin purchase with its number of spins and message): orders name the streamer's PayPal merchant ID as payee so money goes straight to them (never the email: PayPal no longer accepts emails as the SDK's `merchant-id`, and it would be public); pending orders (song, user, price) are kept in memory for three hours. A completed capture whose amount matches queues the song; a pending one (PayPal still reviewing it) does not.
- `song_queue.rs` — requested songs (free and paid) not yet in the streamer's queue, saved to `--song-queue-file`. A background task delivers them whenever the streamer connects, after a request and every 10 s by sending `add` with the request id (random, or the PayPal order id) as `requestId` until Moblin answers `ok`; Moblin's own dedup makes the retries safe. Drops songs Moblin can't find; gives up after 7 days.
- `webhooks.rs` — posts tips and spins to streamers' webhook URLs, retrying every 10 s, six attempts in all, in memory only. Only `https` URLs of public host names, without following redirects, unless `--allow-local-webhooks` (local development, passed by `just backend-run`).
- `twitch.rs` / `live.rs` / `live_status.rs` — Helix client with a cached app access token; a once-per-minute cache of whether the featured channel is live; and a background task that checks which listed channels are live every five minutes and one minute after a live post: all Twitch channels in one streams request, Kick channels one by one with `--lookup-spacing` between them.

Everything optional degrades rather than fails: without Twitch credentials, Twitch profiles come back empty and `/twitch/live` answers 503. `--allow-unattested` disables App Attest and `--fake-twitch-sign-in` signs in as a made-up Twitch user without contacting Twitch, both for local development only (`just backend-run` passes both, and `--allow-local-webhooks`).

## Conventions

- **Do not write comments or docstrings.**
- **Magic numbers are fine, and often preferred, when used in a single place.** Do not hoist a literal
  into a named constant just because it is a literal — only name it when the same value is used in more
  than one place.
- Rust edition 2024 (let-chains are used). Clippy warnings are errors in CI.
- The `/streamers/live` request shape (`channels`, `challenge`, `keyId`, `attestation`, `attestationChallenge`, header `moblin-assertion`) is a contract with the Moblin iOS app; changing it requires an app change too.
- Streamers appear in the list only if they opted in inside the Moblin app. Site copy must not imply that everyone who goes live is listed.

## Reporting

- Never mention styling or formatting changes (oxfmt, cargo fmt output) in summaries, reviews or commit
  messages.
