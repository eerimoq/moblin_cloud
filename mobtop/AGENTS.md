# mobtop

Read the root `AGENTS.md` first. Paths below are relative to `mobtop/`.
This is the Rust ratatui/crossterm client of the same backend used by the website.

| Location | Owns |
| --- | --- |
| `src/main.rs`, `src/app.rs` | Terminal lifecycle, events and application state. |
| `src/backend.rs`, `src/account.rs` | HTTP calls, server configuration and account data. |
| `src/sign_in.rs`, `src/session.rs` | Browser/loopback sign-in and remembered sessions. |
| `src/sse.rs`, `src/feed.rs` | Streaming updates and connection lifecycle. |
| `src/panes/` | Panel implementations; module entry is `src/panes/mod.rs`. |
| `src/layout.rs`, `src/menu.rs`, `src/controls.rs`, `src/ui.rs` | Layout, input and rendering. |
| `src/preview.rs` | Device preview display. |

Use [mobtop architecture](../docs/architecture/mobtop.md) for implementation detail.

Keep provider availability, API messages and permissions aligned with the backend
and website. Do not introduce a second interpretation of a shared contract.

Preserve terminal restoration, task shutdown and loopback sign-in validation.
Never log session tokens. Session writes must preserve other backend entries
and restrictive file permissions; do not report a failed save as successful.

Retain image-protocol fallbacks for terminals without graphics. For controls,
layouts or streams, exercise disconnected, reconnected, unauthorized and missing
device states relevant to the change.

Run `cargo test` and `cargo clippy --all-targets -- -D warnings` for Rust changes.
`just dev-mobtop` uses the fake development stack for interactive checks.
Report terminal/platform coverage actually exercised, and update its architecture
note when behavior or file ownership changes.
