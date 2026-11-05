# System tests

`just system-test [pattern]` runs the black box tests (Python, `systest`, like Moblin's `tests/`): they start the release backend and drive its API like the frontend. Not run in CI; see `README.md`.

- `utils/fake_paypal.py` and `utils/fake_stripe.py` are stdlib only and started once per run, so their orders survive backend restarts. `--paypal-url` and `--stripe-url` point the backend at them, and a PayPal URL other than the live one is the sandbox.
- `utils/fake_twitch.py` and `utils/fake_kick.py` are stdlib only. A login ending in 1 to 3 is subscribed at that tier on Twitch and has a subscriber badge on Kick, and any login but `nobody` resolves to a user. User ids are hashed from the login, so the same login on both fakes is two users until linked.
- With `just dev-backend`, `admin` is an admin, `premium` a premium account and `normal` neither.
- `utils/fake_moblin.py` follows Moblin's semantics and refusals for the status, controls, preview and scene editor requests; `just moblin-drift` tracks it. Its made-up settings are in `utils/fake_moblin_database.py`.
- The fake Moblin's wheel changes when the widget picked for Moblin Cloud changes, either through the tester's `change_wheel` or `setMoblinCloudWheelOfLuckWidget`.
- The relay in `utils/moblin.py` closes with 1000 when the tester changes Moblin's settings and 1001 otherwise, so its `disconnect()` acts like a dropped connection.
- `just dev-fake-moblin` connects the fake to a running `just dev-backend` as the admin user's Moblin, with `--key` taken from the dev database, so sign in as `admin` once first.
