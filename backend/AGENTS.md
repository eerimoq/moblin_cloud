# Backend

Read the root `AGENTS.md` first. This is a Rust 2024 Axum service. The API, device
WebSocket connections and durable SQLite state live here.

## Ownership

- `src/main.rs`: CLI/environment parsing, integration construction and shutdown.
- `src/api.rs`: route registration, shared authentication/permission helpers and
  middleware. `src/api/*.rs` owns endpoint behavior and inline API tests.
- `src/api/public.rs`: public page responses and server configuration.
- `src/auth.rs`: OAuth state, PKCE, sessions, linking and terminal sign-in.
- `src/accounts.rs`: identities, ownership, assistant access and account state.
- `src/database.rs`: SQLite creation/opening and shared connection access.
- `src/payments.rs`, `src/api/orders.rs`, `src/paypal.rs`, `src/stripe.rs` and
  `src/deliveries.rs`: order state, provider operations, reconciliation and delivery.
- `src/remote_control.rs`: Moblin protocol and connection state.
- `src/app_attest.rs`: device attestation. Do not relax it to repair unrelated bugs.
- `src/api/test_utils.rs` and `src/api/test_utils/`: isolated API fixtures and fakes.

## Rules

Reuse the existing ownership, assistant-rights and admin helpers. Test that one
account cannot access another's page/device, including every mutation and stream.
Do not treat CORS as authentication or provider availability as a permission.

Validate untrusted JSON, identifiers, amounts, URLs and uploaded media at their
boundary. Keep request/stream limits and timeouts. Never log session or device
keys, OAuth credentials, raw authorization headers or attestation material.

The SQLite database uses WAL. Test persistence across reopen/restart. Creating a
table if absent does not migrate an existing table: plan an explicit upgrade and
test an older database copy before changing stored schemas. Never overwrite or
experiment on a production database.

Preserve transactional payment recording, conditional state transitions,
idempotency and recovery from lost provider responses. Keep existing captures,
webhooks and settlement available when new public orders are disabled. Exercise
declines, duplicate requests, provider failures, shutdown and restart with fakes.

Moblin messages and remote-control permissions are shared contracts. Keep the
Swift client, browser, mobtop and fake Moblin in sync when changing a message.

## Verification

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

Run targeted API tests during development and `just system-test` when end-to-end
flows change. Use the repository's Rust style check. Keep inline test modules
after production items so strict Clippy passes. Do not run live payments or
attestation experiments without explicit authorization.
