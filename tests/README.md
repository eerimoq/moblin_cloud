Black box system tests of the backend. They build and start it, then drive its API like the
frontend does. Everything around it is faked: PayPal (`utils/fake_paypal.py`), Stripe
(`utils/fake_stripe.py`), Twitch (`utils/fake_twitch.py`), Kick (`utils/fake_kick.py`) and Moblin
(`utils/fake_moblin.py`). The tester acts as the fake Moblin's remote control assistant and relays
its Moblin Cloud connection to the backend, which is how the tests see what the backend delivers.

Run from the repository root:

```bash
pip install -r requirements.txt
playwright install chromium
just system-test
just system-test Tip
just system-test --interactive
```

No configuration is needed. To sign in with a real Twitch account, copy
`tests/config.example.toml` to `tests/config.toml` (or
`$XDG_CONFIG_HOME/moblin-cloud/tests/config.toml`) and fill in `[twitch]`; `--interactive` runs
the tests needing a person, like that sign in, and shows the browser.
