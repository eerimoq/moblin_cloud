# AGENTS.md

This file provides guidance to coding agents when working with code in this repository.

## What this is

Moblin Cloud (cloud.moblin.app): the streamers' tip pages, where viewers tip them and spin their
wheel of luck, the dashboard where they manage them, and the connection to their Moblin devices.
The landing page for Moblin (moblin.app) is its own repository, `moblin_website`.

- `frontend/` — a static site (SolidJS + TypeScript + Tailwind v4 + Vite), deployed to GitHub Pages.
- `backend/` — a small Rust (axum) service deployed at `https://cloud-api.moblin.app` as a Docker image. It serves the pages, the dashboard and Moblin's Moblin Cloud WebSocket.
- `mobtop/` — a terminal monitor (Rust, ratatui + crossterm), the web Monitor page in a terminal, talking to the backend's API like the frontend.
- `deploy/` — runs a copy of the frontend and backend on one server with Docker Compose, from the images the deploy workflow publishes on every tag; see its README.

## Where the details are

Read the file for the part you are working on before changing it:

- `frontend/AGENTS.md` — the frontend's tests, build and rules.
- `frontend/src/components/sceneEditor/AGENTS.md` — the scene editor.
- `frontend/src/components/streamSettings/AGENTS.md` — the stream settings.
- `backend/AGENTS.md` — the backend's modules, payments, Moblin sessions, sign in and running a single test.
- `backend/src/api/AGENTS.md` — the routes and who may call them.
- `mobtop/AGENTS.md` — running mobtop and its pitfalls.
- `tests/AGENTS.md` — `just system-test [pattern]` and the fakes.
- `deploy/AGENTS.md` — what routing and backups have to keep.

## Commands

All checks are driven by the `justfile` at the repo root:

```
just style          # oxfmt (frontend) + cargo fmt (backend, mobtop)
just style-check
just lint           # oxlint --deny-warnings + cargo clippy --all-targets -D warnings (backend, mobtop)
just spell-check    # codespell, configured in .codespellrc
just test           # vitest (frontend) + cargo test (backend, mobtop)
```

CI runs these, with `just style-check` instead of `just style`. It runs neither `npm run build` nor `just system-test`, so a passing CI says nothing about the frontend's type errors or the system tests.

`just dev` starts the backend, the frontend, signs in as `admin` on the fake Twitch and runs the fake Moblin. `just dev kick` picks the fake sign in platforms (`twitch,kick` by default, `just dev ""` none). The development recipes are in the `dev` group of `just --list`. In two terminals instead:

```
just dev-backend    # starts the fake Twitch and Kick, then cargo run --allow-unattested with backend/database.sqlite
just dev-frontend   # vite dev server; proxies /api -> localhost:8080
```

The `dev` recipes are thin wrappers around `scripts/dev.py`. They run the fakes in their own process and start the backend and the frontend each in a new session, killing the whole process group on exit: killing only `cargo run` or `npm` leaves the backend or vite running.

`just moblin-drift` (needs Moblin checked out at `../moblin`) lists, per port, the Moblin commits on the `remote-control-scenes-and-widgets` branch (`--ref` for another) not yet ported; `--diff` shows the Swift diffs and `--mark <file>...` marks files as ported. Only mark a file once its Swift changes are ported, never to silence the script's errors about missing files or commits. `scripts/moblin_ports.json` maps each scene editor and stream settings file, the backend's `remote_control.rs` and the fake Moblin to the Swift they port, so a new file in either needs an entry. A port with its own `branch` is compared with that Moblin branch instead of the default one.

## Rules across the projects

- Moblin's remote control messages are a contract with the Moblin iOS app and must match `Moblin/RemoteControl/RemoteControl.swift`; changing them requires an app change too.
- The WebSocket, Twitch and Kick callback and PayPal webhook paths are registered outside this repo (in Moblin and in Twitch's, Kick's and PayPal's consoles) and must not be renamed. `GET /pages/addresses` is a contract with the Moblin website's backend.
- Tables are created with `CREATE TABLE IF NOT EXISTS`, which never changes an existing table, so a column added to one needs a step in `migrate` (`backend/src/database.rs`).
- The web Monitor and mobtop do the same things and read and write the same saved layout, so a change to one usually needs the other.
- Nothing is polled: what changes on the backend is pushed as server-sent events. The one exception is mobtop waiting for a sign in on another device.
- There is no fake logic in the backend: the fakes are in `tests/utils/`, and the options pointing the backend at them and shortening its timers are for local development and the system tests only.
- Viewer-facing backend errors are full sentences the page shows as they are, and stay English like the Terms and admin pages.
- An assistant must never get a streamer's Moblin Cloud key or payment details.
- A tag publishes the Docker images and a push to main deploys the frontend, so never create a tag or dispatch a deploy workflow to test a change.

## Conventions

- Do not write comments or docstrings.
- Magic numbers are fine, and often preferred, when used in a single place. Only name a literal when the same value is used in more than one place.
- Rust edition 2024 (let-chains are used). Clippy warnings are errors in CI.
- Rust unit tests are never inline: a module's `tests` module is `tests/<module>.rs` in the module's directory, declared with `#[path]` (relative to the declaring file's directory, so `api.rs` writes `api/tests/...`), and a type's test-only helpers are an `impl` block in there, not `#[cfg(test)]` items in the module. A file loaded through `#[path]` owns its directory, so its own submodules are its siblings.
- Only write in the `AGENTS.md` files what the code can't tell: rules, contracts, pitfalls and the reasons behind them, not routes, layouts or how a feature behaves.

## Reporting

- Never mention styling or formatting changes (oxfmt, cargo fmt output) in summaries, reviews or commit messages.
- Say when a flow only ran against the fakes: that does not verify the real sign in, payment or Moblin.
