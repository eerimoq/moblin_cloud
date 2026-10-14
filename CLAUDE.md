# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

The landing page for Moblin (moblin.app) and its companion apps. Two independent
projects live here:

- `frontend/` — a static site (SolidJS + TypeScript + Tailwind v4 + Vite), deployed to GitHub Pages.
- `backend/` — a small Rust (axum) service deployed at `https://api.moblin.app` as a Docker image. It serves the "Streamers" list and the "Erik is live on Twitch" status.

`README.md` documents the deploy flow, the App Attest scheme and the Twitch credentials; read it before touching either.

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

- **Multi-page, no router.** Each page is its own HTML entry listed in `vite.config.ts` (`index.html`, `chat-bot/index.html`, `song-requests/index.html`, ...) with its own `src/*.tsx` entry that renders a page component. Sub pages live in a directory so GitHub Pages serves clean URLs like `/chat-bot/`. Add a page by adding all three: the HTML file, the entry, and the `rollupOptions.input` line. `404.html` is what GitHub Pages serves for unknown paths; it renders the song request page for `/song-requests/<name>` and a not found page otherwise.
- **Base path is dynamic.** `vite.config.ts` uses `/` when `public/CNAME` exists and `/moblin_website/` otherwise. Anything referencing a file under `public/` must go through `asset()` in `src/asset.ts`; never hardcode a leading slash in a src attribute.
- **Content lives in `src/data/`, not in components.** URLs in `links.ts`, products/features/reviews/chat-bot commands in their own files. Components in `src/components/` are one section each and are composed in order in `App.tsx`.
- **Backend access** goes through `data/backend.ts`: `/api` in dev (proxied), `links.api` in production. `data/streamers.ts` fetches the list once as a module-level SolidJS resource; `twitch.ts` polls `/twitch/live` every five minutes and falls back to "not live" on any error.
- **Theming** is Tailwind v4 `@theme` tokens in `src/index.css` (e.g. `bg-card`, `text-leaf`, `font-display`). Dark is the default; light is the same token names overridden under `prefers-color-scheme: light`. Use the tokens, not raw colors.
- Icons are inline stroke SVGs in `components/Icon.tsx` keyed by `IconName`; platform logos on a colored disc are `BrandIcon`.

## Backend architecture

One binary, `backend/src/main.rs`, wired together from:

- `api.rs` — the axum router. Routes: `GET /streamers`, `POST /streamers/live/challenge`, `POST /streamers/live`, `GET /twitch/live`, `POST /remote-control/streamers`, `GET /remote-control/streamer/{key}` (WebSocket), `GET`/`POST /remote-control/streamer/{key}/settings`, `GET /remote-control/names/{name}`, `GET`/`POST /remote-control/{id}/music`. CORS allows only `https://moblin.app` (hardcoded).
- `store.rs` — in-memory list of streamers, newest post first. It keeps the `--max-streamers` most recent posters (so a newcomer is still around when the live check runs a minute later) plus any older streamer who is still live, up to another `--max-streamers`; `streamers()` lists live streamers first, capped at `--max-streamers`. A streamer is identified by sharing any channel (platform + case-insensitive name); going live again moves to the top and keeps any already-looked-up profile and the live flag. Each listed channel carries a `Lookup` (pending with retry backoff, or done) that serializes flat into the channel JSON as `avatar`/`displayName`, plus `live` (only ever true for Twitch and Kick channels).
- `profiles.rs` — a background task that pulls the next due lookup from the store and resolves display name and avatar per platform (Twitch via Helix, YouTube via `og:` meta tags with unavatar.io fallback, Kick via `kick.rs`, its public channels API), with `--lookup-spacing` between requests.
- `app_attest.rs` — verifies Apple App Attest attestations and assertions so only the Moblin app can post live. Stateless by design: the app resends its attestation every time and the server re-verifies the chain each time, deliberately ignoring certificate validity dates. Tests use public sample vectors in `src/testdata/`.
- `challenges.rs` — one-time 32-byte challenges, 120 s lifetime, that every live post must consume.
- `remote_control.rs` — the assistant side of Moblin's remote control protocol, for the song request page. Streamers connect with a secret key in the URL; the public page id is derived from it. Serde types mirror Swift's synthesized `Codable` (`{"ping":{}}`, optional fields omitted), and must match `Moblin/RemoteControl/RemoteControl.swift` in the Moblin repo. Only `music` status and add requests are sent.
- `song_requests.rs` — per streamer song request settings, keyed by the public id: whether requests are on and an optional unique page name. Saved to `--song-requests-file` (JSON, written atomically) on every change, in memory only without it.
- `twitch.rs` / `live.rs` / `live_status.rs` — Helix client with a cached app access token; a once-per-minute cache of whether the featured channel is live; and a background task that checks which listed channels are live every five minutes and one minute after a live post: all Twitch channels in one streams request, Kick channels one by one with `--lookup-spacing` between them.

Everything optional degrades rather than fails: without Twitch credentials, Twitch profiles come back empty and `/twitch/live` answers 503. `--allow-unattested` disables App Attest for local development only.

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
