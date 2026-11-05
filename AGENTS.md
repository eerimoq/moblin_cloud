# Moblin Cloud: shared agent guidance

Read this file first, then the nearest folder's `AGENTS.md`. The guidance is for
all coding agents. Prefer the current source and manifests over dated examples.

## Product and architecture

Moblin Cloud provides streamer dashboards, public viewer interactions, device
monitoring and remote control of Moblin. The Moblin marketing website is a
separate project; do not replace this application's landing page with that site.

Three clients share one backend:

```text
SolidJS website ─┐
mobtop terminal ├─ Rust HTTP/SSE backend ─ SQLite
Moblin devices ─┘          ├─ device WebSocket connections
                          └─ Twitch, Kick, PayPal and Stripe integrations
```

The frontend is a static multi-page application, not a server-rendered app or a
client-router framework. The backend owns authentication, permissions, payments
and durable state. `mobtop` is an API client, not a second backend.

## Find the owning code

| Area | Entry points and responsibilities |
| --- | --- |
| `frontend/` | Vite HTML entries, SolidJS UI, TypeScript API clients, Tailwind tokens, translations and browser streams. Read `frontend/AGENTS.md`. |
| `backend/` | Axum HTTP routes, SQLite state, OAuth, provider integration and device protocol. Read `backend/AGENTS.md`. |
| `mobtop/` | Rust terminal UI, backend client, session persistence and SSE. Read `mobtop/AGENTS.md`. |
| `tests/` | Python black-box scenarios and fake Twitch, Kick, payment and Moblin services. Read `tests/AGENTS.md` and `tests/README.md`. |
| `deploy/` | Portable Compose configuration and operator setup. Read `deploy/AGENTS.md`. |
| `scripts/` | Translation maintenance and tracking ports from Moblin Swift source. |
| `.github/workflows/` | CI, GitHub Pages publication and tagged container-image publication. |
| `.config/`, `.codespellrc` | Existing Python and spelling tool configuration. |
| `justfile` | Shared development and verification commands. |

Detailed implementation notes are in `docs/architecture.md`. Consult the relevant
section before changing a subsystem. `CLAUDE.md` is a compatibility pointer to
this shared guidance; keep instructions here rather than in competing copies.

## Work safely and keep scope small

- Check Git status and the active branch before editing. Preserve unrelated work.
- Read the owning implementation and every caller before changing a contract.
- Keep upstream contributions generic. Instance branding, organization-specific
  access rules and host provisioning do not belong in a reusable application PR.
- Preserve the existing architecture and dependencies. Do not introduce a router,
  framework, ORM, service or generalized abstraction without a concrete need.
- Follow upstream's convention against adding code comments and docstrings;
  explain behavior in names, tests and documentation.
- Leave one-off literals inline; introduce named constants when values are reused.
- Never commit, print or copy live `.env`, OAuth/session secrets, device keys,
  user databases, payment details or private test configuration.
- Do not deploy, release, tag images, modify accounts, use real payments or change
  external infrastructure without explicit authorization.
- Keep unrelated changes and instruction-documentation work in separate PRs.
- Do not rewrite another contributor's commits or force-push without authorization.

## Shared contracts

- `GET /config` is the existing source for configured sign-in and payment
  providers. Its UI availability flags are public; credentials are not. Reuse it
  instead of adding another provider-discovery endpoint or credential logic.
- UI visibility is not authorization. Backend checks remain authoritative for
  account ownership, admin access, assistant rights and device controls.
- Keep frontend runtime `config.js` distinct from backend `/config`. The former
  supplies public browser settings; the latter describes server integrations.
- Presentation and visibility are not access policies. Keep effective feature
  capabilities consistent between clients and the backend. Disabling new
  interactions must not strand existing captures, webhooks or reconciliation.
- HTTP, SSE and WebSocket paths are separate interfaces. A browser-only login
  gate must not silently break phone or terminal clients.
- Moblin JSON messages are a contract with the iOS app. Coordinate changes with
  the Swift source, both clients and the fakes; preserve compatibility.
- Preserve the exact-once recording/delivery and recovery behavior of payment
  state transitions. Do not weaken validation or exception handling to pass tests.

## Development and checks

Install prerequisites from `README.md` and `requirements.txt`, with versions
consistent with the manifests, Dockerfiles and CI. Use the lockfiles. This repo
uses npm for its frontend and separate Cargo projects for both Rust programs.

```sh
cd frontend
npm ci
cd ..
just dev
```

Use `just dev kick` to exercise Kick-only configuration and `just dev ""` for
no sign-in providers. The default development setup uses fake services, not live
OAuth or payment accounts.

Run the smallest meaningful checks for the touched area, then the applicable
shared checks before marking a PR ready:

```sh
just style-check
just lint
just spell-check
just test
```

Frontend changes also require `cd frontend && npm run build`; unit tests do not
replace TypeScript checking or a production build. Browser-facing changes need
an actual preview check, including a narrow layout and relevant empty/error
states. Do not claim browser tests or live provider integration passed from
unit tests alone.

Use `just system-test` for changes to end-to-end API, payment recovery or device
behavior. Its prerequisites and fake-service boundaries are in `tests/README.md`;
it is not part of the current CI workflow. Never run interactive real-account
tests without explicit authorization.

For scene-editor or remote-control ports, inspect `scripts/moblin_ports.json` and
run `just moblin-drift` with the appropriate Moblin checkout/ref. Do not mark a
port current without reviewing the corresponding Swift changes.

Do not automatically run `just update-dependencies` or `just machine-translate`:
they broadly change inputs or use an external service. Ask for a scoped request
when either is needed.

## Finish

Run `git diff --check`, inspect the complete diff and report what changed, which
behavioral tests passed, and any checks that remain blocked. Omit styling or
formatting changes from summaries, reviews and commit messages. Distinguish source
review, simulated integrations and live validation. A queued or approval-blocked
workflow is not a passing workflow. Keep dates and transient release/CI status
in the PR or review record rather than these instructions.
