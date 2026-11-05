# System tests and fake services

Read the root `AGENTS.md` and [tests/README.md](README.md). These Python black-box
scenarios are separate from the inline Rust tests and frontend Vitest tests.

## Fixtures and ownership

`tests/utils/` contains fake Twitch, Kick, PayPal, Stripe and Moblin services.
They emulate protocol behavior and failures; they are not alternate production
implementations. Keep fakes aligned with the real contract when endpoints or
Moblin messages change.

Use isolated accounts and temporary databases. Preserve provider/order state
needed by restart scenarios. Never test against an operator's database or real
payment account by default. Private `tests/config.toml` and user-level test
configuration may contain credentials; do not print or commit them.

## Running scenarios

Run `just system-test [pattern]` from the repository root. For example,
`just system-test Tip` selects the relevant payment scenarios. Prerequisites and
browser setup are documented in `tests/README.md`; do not copy that setup here.

Interactive real-provider tests are distinct from the default fake flows.
Use them only within the requested scope, and do not describe fake success as
a real OAuth, payment or phone verification.

For a regression, add the smallest scenario that would fail before the fix.
Check refused operations, ownership/permission boundaries and recovery when
relevant. Avoid fixtures that simply reproduce the implementation.

Use the existing isort, Ruff, pylint and mypy configuration through the root
recipes for Python edits. Update `tests/README.md` when test setup changes and
the owning subsystem note when a fake models a changed contract.
