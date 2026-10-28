Black box system tests of the backend. They build and start the backend, then use its API
like the website does: a streamer signs in and manages the page in the dashboard, and viewers
tip and spin the wheel on the page. Payments go through fake PayPal and Stripe servers.
Everything but the frontend is tested.

The streamer's Moblin is the fake one in `tests/utils/fake_moblin.py` by default, which speaks
Moblin's remote control protocol, both as its remote control streamer and as Moblin Cloud.
Give `--device` to test with a real Moblin device instead. Streamers and viewers sign in with
the fake Twitch in `tests/utils/fake_twitch.py` by default, which the backend is pointed at with
`--twitch-sign-in-url` and which signs any made-up login in; `just dev-backend` starts the same
one for development.

All commands below are run from the repository root.

# Prerequisites

```bash
pip install -r requirements.txt
playwright install chromium
```

# Configuration

The tests run on this machine with the fake Moblin without a configuration. For Twitch or a real
Moblin device, copy `tests/config.example.toml` to `tests/config.toml` and modify it
to match your test setup. `tests/config.toml` is used if it exists, otherwise
`$XDG_CONFIG_HOME/moblin-website/tests/config.toml`.

Without `[twitch]` the Twitch lookups are skipped, and without `--device` the tests needing a real
Moblin.

## PayPal and Stripe

Payments always go through the fake PayPal in `tests/utils/fake_paypal.py` and the fake Stripe in
`tests/utils/fake_stripe.py`, which the backend is pointed at with `--paypal-url` and
`--stripe-url`. Their onboarding links connect a made-up seller or account at once and return to
the dashboard. Viewers pay with a test card through their APIs, like in PayPal's popup or Stripe's
Payment Element. The payment tests run once with PayPal and once with Stripe, the Stripe ones
named with a `Stripe` suffix.

# Real Moblin device configuration

Moblin's remote control streamer connects to the tester at `remote-control-port`. The tester
acts as Moblin's assistant and sets Moblin up by importing settings. When a test gives Moblin a
Moblin Cloud key from the dashboard, Moblin Cloud connects to the tester at the same port too, and
the tester relays that connection to the backend while watching what they send each other. That
is how the tests see the tips and wheel spins the backend delivers.

This is only needed for `--device`.

1. Build Moblin in debug with `MOBLIN_CLOUD_API_URL` set to
   `http://<tester-ip-address>:<remote-control-port>` in `Config/User.xcconfig`, so Moblin
   cloud connects to the tester.
2. Generate settings into clipboard.
   ```bash
   just system-test-device-settings
   ```
3. Import the generated settings from clipboard into Moblin.

# Run the tests

```bash
just system-test
just system-test Tip
just system-test --device iphone16pro
just system-test --device iphone16pro --interactive
```

`--interactive` also runs the tests needing a person, like signing in with a real Twitch account
in the browser or changing the wheel on a real Moblin device.
