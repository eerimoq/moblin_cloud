# System tests and fake services

Read the root `AGENTS.md` and `tests/README.md` first. These Python black-box
scenarios start the backend and fake external services. They are separate from
the inline Rust unit/API tests and frontend Vitest checks.

The fake Twitch and Kick implement test sign-in; fake PayPal and Stripe implement
payment behavior; fake Moblin emulates device connections and remote-control
requests. Keep fakes consistent with real protocol semantics, including failures.
Do not add fake-service shortcuts to production handlers.

Use temporary databases and isolated accounts. Keep restart tests' required
provider/order state alive for the scenario. Never read or write real operator
databases or use live payment credentials by default.

`tests/config.toml` and user-level test configuration can contain credentials.
Do not print, commit or copy them into reports. The tracked example configuration
is the place for nonsecret setup documentation.

Run from the repository root:

```sh
just system-test
just system-test Tip
```

Install prerequisites and browser dependencies using `tests/README.md` when
needed. `--interactive` and real-provider tests require an explicit request; a
successful fake flow is not proof of a real OAuth or payment integration.

For Python edits, use the existing isort, Ruff, pylint and mypy configuration
through the root `just` recipes. Cover refused operations and recovery, not only
the happy path. Reproduce each regression with the smallest relevant scenario.
