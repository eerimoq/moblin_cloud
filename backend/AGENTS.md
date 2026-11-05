# Backend

Read the root `AGENTS.md` first. Paths below are relative to `backend/`.

## Find the implementation

| Location | Owns |
| --- | --- |
| `src/main.rs` | CLI/environment parsing, integration construction and shutdown. |
| `src/api.rs`, `src/api/` | Routes, shared permission helpers, endpoint behavior and inline API tests. |
| `src/api/public.rs` | Public page responses and server configuration. |
| `src/auth.rs`, `src/accounts.rs` | OAuth, sessions, linking, ownership and assistant access. |
| `src/database.rs`, `src/pages.rs` | SQLite access and persisted page settings. |
| `src/payments.rs`, `src/api/orders.rs` | Order transitions and capture/recovery orchestration. |
| `src/paypal.rs`, `src/stripe.rs`, `src/deliveries.rs` | Provider calls and durable device delivery. |
| `src/remote_control.rs`, `src/app_attest.rs` | Moblin protocol, connection state and attestation. |
| `src/api/test_utils.rs`, `src/api/test_utils/` | Isolated servers and external-service fakes. |

Use the [architecture index](../docs/architecture.md) to select API, account,
assistant, storage, device, payment or subscriber notes.

## Preserve trust and persistence boundaries

Reuse the existing `signed_in`, `admin` and page/assistant permission helpers.
Test ownership and rights for mutations and streams, including denied access.
CORS and provider availability are not authorization.

Validate untrusted JSON, identifiers, amounts, URLs and media at their boundary.
Keep limits, timeouts and error handling. Never log authorization headers,
session/device keys, OAuth credentials or attestation material. Do not relax
attestation to debug an unrelated failure.

SQLite uses WAL and a shared connection. Preserve transactions and test reopen/
restart behavior for stored changes. `CREATE TABLE IF NOT EXISTS` does not update
an existing table: an existing-schema change needs an upgrade path and an
old-database test. Use an isolated copy, not an operator's database.

Capture recording and associated delivery insertion are transactional and
idempotent. Delivery to Moblin is at least once, with retry/deduplication through
`requestId`; do not replace this with an exactly-once assumption. Preserve lost
provider-response recovery, duplicate-capture handling and settlement behavior.

Protocol changes affect the iOS app, browser, mobtop and fake Moblin. Update
affected clients and fixtures together. Keep fake-provider behavior out of
production handlers; use the existing test overrides.

## Check the change

Run targeted inline tests during development, then `cargo test` and
`cargo clippy --all-targets -- -D warnings` for backend changes. Keep inline test
modules after production items. Root recipes own the other shared checks.

For changed end-to-end behavior, use a relevant `just system-test [pattern]`
scenario. Cover failed/refused operations and recovery as well as success.
Update the owning architecture note when a contract or mechanism changes.
