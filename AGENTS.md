# Moblin Cloud: shared agent instructions

## Read before working

These instructions are shared by Claude, Codex and other coding agents. Read this
file, then explicitly read the `AGENTS.md` in each folder you will change. Do not
assume your client loaded nested guidance. The folder rules apply to that subtree.

Start by checking Git status, the active branch and the relevant implementation.
Preserve existing work and follow the requested Git workflow; this repository
does not require agents to create a PR or use a particular branch naming scheme.

Claude startup instructions are in [README.md](README.md#coding-agents). Keep
`AGENTS.md` as the only instruction filename; do not add model-specific copies,
compatibility stubs or instruction symlinks.

## What lives here

Moblin Cloud is the dashboard, viewer pages and device connection service, not
the separate Moblin marketing website. The web frontend and mobtop terminal
client use the Rust backend; Moblin devices connect through its WebSocket
protocol. The backend stores durable state in SQLite and integrates with Twitch,
Kick, PayPal and Stripe.

| Folder | Read next | Responsibility |
| --- | --- | --- |
| `frontend/` | [frontend/AGENTS.md](frontend/AGENTS.md) | Static SolidJS/TypeScript site, multi-page Vite entries, styles, translations and browser streams. |
| `backend/` | [backend/AGENTS.md](backend/AGENTS.md) | Axum API, accounts, SQLite, payments and Moblin connections. |
| `mobtop/` | [mobtop/AGENTS.md](mobtop/AGENTS.md) | Rust terminal UI and client of the same backend. |
| `tests/` | [tests/AGENTS.md](tests/AGENTS.md) | Python black-box scenarios and fake external services. |
| `scripts/` | [scripts/AGENTS.md](scripts/AGENTS.md) | Translation and tracking ports from Moblin's Swift source. |
| `deploy/` | [deploy/AGENTS.md](deploy/AGENTS.md) | Portable Compose setup and proxy configuration. |
| `.github/` | [.github/AGENTS.md](.github/AGENTS.md) | Verification and publication workflows. |

The [architecture index](docs/architecture.md) links to subsystem notes. Read the
relevant note and source files rather than loading all implementation notes.

## Existing project conventions

- Do not write comments or docstrings.
- **Magic numbers are fine, and often preferred, when used in a single place.**
  Do not hoist a literal into a named constant just because it is a literal; only
  name it when the same value is used in more than one place.
- Rust uses edition 2024. Clippy warnings are errors.
- Moblin's remote-control messages are a contract with the iOS app. Coordinate
  protocol changes with the app, web client, mobtop and fake Moblin.

Keep changes focused and reuse existing code. Read callers before changing a
shared function or response. Do not replace the application's architecture or
add dependencies to solve a local problem without a concrete reason.

## Boundaries to preserve

- Reuse `GET /config` for configured sign-in and payment providers. Credentials
  stay on the backend; public availability flags do not grant permission.
- Keep public browser `config.js` distinct from the backend configuration API.
- Enforce ownership, assistant rights and admin access on the backend. Hiding a
  control is not authorization, and CORS is not authentication.
- Capture recording is transactional and idempotent. Device delivery is
  deliberately **at least once**: retries reuse `requestId`, and Moblin
  deduplicates it. Do not promise exactly-once network delivery.
- Preserve recovery from lost provider responses and failed/retried deliveries.
  Do not weaken validation, permissions or error handling to make a test pass.
- Keep secrets, device keys, sessions, real databases and private test
  configuration out of logs, commits and reports. Use isolated fakes for tests.
- Do not deploy, publish releases/images or operate real accounts/payments unless
  those actions are part of the user's request.

## Commands and verification

The `justfile` is the executable source of truth for shared commands. See
`README.md` for prerequisites and `tests/README.md` for system-test setup.
Use the repository's npm and Cargo lockfiles and existing tool configuration.

| Change | Relevant verification |
| --- | --- |
| Frontend logic or UI | `cd frontend && npm test`; `npm run build` there for TypeScript and production bundling. |
| Backend | `cd backend && cargo test`; `cargo clippy --all-targets -- -D warnings` there. |
| mobtop | Its `cargo test` and strict Clippy checks. |
| Cross-client/API/payment behavior | Relevant fake-service scenario with `just system-test [pattern]`. |
| Python | Applicable root style/lint recipes using the existing configuration. |
| Documentation | Spelling, local links, paths and `git diff --check`. |

Root check recipes are `just style-check`, `just lint`, `just spell-check` and
`just test`. Run checks appropriate to the change; do not run the entire stack
for a prose-only edit. System tests and the frontend production build are not
currently part of the CI test recipe. Do not infer that they passed from CI.

For UI changes, use an available browser to check the affected states. Report
missing browser or real-device coverage rather than inventing results. A fake
OAuth/payment flow is not a live-provider verification.

## Keep the guidance useful

When changing behavior, endpoints, ownership or file locations, update the
affected subsystem note and folder guidance in the same change. The source wins
when a note is stale. Describe mechanisms in architecture notes; keep commands
in the `justfile` and agent workflow rules here or in the owning folder's
`AGENTS.md`. Do not copy command catalogs or policy into architecture notes.

Do not turn temporary CI results, machine-specific problems, PR numbers or
unmerged features into standing instructions. Preserve unrelated local changes.
If a required tool/check is unavailable, report the limitation and continue
unaffected work without weakening safeguards.

## Report

Explain the resulting behavior, relevant tests and remaining limitations. Never
mention styling or formatting changes in summaries, reviews or commit messages.
A queued workflow or one requiring approval is not a passing workflow.
