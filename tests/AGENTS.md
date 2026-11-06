# System tests

`just system-test [pattern]` runs the black box tests (Python, `systest`, like Moblin's `tests/`): they start the release backend and drive its API like the frontend. Not run in CI; see `README.md`.

- `utils/fake_paypal.py` and `utils/fake_stripe.py` are stdlib only and started once per run, so their orders survive backend restarts. `--paypal-url` and `--stripe-url` point the backend at them, and a PayPal URL other than the live one is the sandbox.
- `utils/fake_twitch.py` and `utils/fake_kick.py` are stdlib only. A login ending in 1 to 3 is subscribed at that tier on Twitch and has a subscriber badge on Kick, and any login but `nobody` resolves to a user. User ids are hashed from the login, so the same login on both fakes is two users until linked.
- With `just dev-backend`, `admin` is an admin, `premium` a premium account and `normal` neither.
- `utils/fake_moblin.py` follows Moblin's semantics and refusals for the status, controls, preview and scene editor requests; `just moblin-drift` tracks it. Its made-up settings are in `utils/fake_moblin_database.py`.
- The fake Moblin's wheel changes when the widget picked for Moblin Cloud changes, either through the tester's `change_wheel` or `setMoblinCloudWheelOfLuckWidget`.
- The relay in `utils/moblin.py` closes with 1000 when the tester changes Moblin's settings and 1001 otherwise, so its `disconnect()` acts like a dropped connection.
- `just dev-fake-moblin` connects the fake to a running `just dev-backend` as the admin user's Moblin, with `--key` taken from the dev database, so sign in as `admin` once first.
- The fakes only misbehave when a test tells them to, through the fake itself (`down`, `plan_capture`, `refunds`, the seller switches). They are shared by every test and outlive backend restarts, so a test that changes one calls `reset()` in its teardown.
- The backend is a black box: a test only reaches it like the frontend, Moblin and the providers do, never through its database or files. What takes the backend minutes is tested by shortening its timers with its hidden options instead: every test runs with deliveries retried and stream access rechecked every second and 2 seconds for Moblin to answer a control, and the deadline tests restart it with `--payment-deadline` and `--settle-interval`, since a short deadline would fail the other payment tests.
- A test that starts the backend with other options (`restart(without=..., extra=...)`) restarts it without any in its teardown, or every later test runs with them.
- The backend is killed with SIGKILL unless a test stops it with `stop(graceful=True)`, so only those tests run the backend's shutdown.
- A killed backend writes no coverage profile, so with `--coverage` every stop is graceful: a test of the backend being killed only tests a clean restart there, and the shutdown code shows as run by every restart. Only the backend's own crate is instrumented, not its dependencies.
