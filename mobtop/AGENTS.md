# mobtop

Read the root `AGENTS.md` first. mobtop is the Rust ratatui/crossterm terminal
client of the same backend used by the web application.

`src/main.rs` owns terminal initialization and event processing. `backend.rs`
owns HTTP integration, `sign_in.rs` owns browser/loopback sign-in, `session.rs`
stores sessions, `sse.rs`/`feed.rs` handle updates, and `app.rs`, `panes.rs`,
`layout.rs`, `menu.rs` and `controls.rs` coordinate the interface.

Use backend configuration for available providers. Keep API messages, permissions
and behavior aligned with the browser rather than inventing a second contract.

Preserve terminal restoration on errors, task shutdown and loopback sign-in
validation. Never log session tokens. Keep session-file permissions restrictive
and preserve other backend entries when updating a session. A write failure must
not be reported as a successful save.

Changes to remote controls, layouts or streams need tests for disconnected,
reconnected, unauthorized and unavailable-device states. Keep image-protocol
fallbacks working in terminals without graphics support.

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

Use `just dev-mobtop` against the fake development stack for interactive checks.
Describe untested terminal/platform combinations honestly.
