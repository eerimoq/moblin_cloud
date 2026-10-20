Black box system tests of the backend. They build and start the backend, then use its API
like the website does: a streamer signs in and manages the page in the dashboard, and viewers
tip, spin the wheel and request songs on the page. Payments go through the PayPal sandbox and Stripe's test mode.
Everything but the frontend is tested.

The streamer's Moblin is the fake one in `tests/utils/fake_moblin.py` by default, which speaks
Moblin's remote control protocol, both as its remote control streamer and as Moblin Cloud.
Give `--device` to test with a real Moblin device instead. Streamers and viewers sign in with
the fake Twitch in `tests/utils/fake_twitch.py` by default, which the backend is pointed at with
`--twitch-sign-in-url` and which signs any made-up login in; `just backend-run` starts the same
one for development.

All commands below are run from the repository root.

# Prerequisites

```bash
pip install -r requirements.txt
playwright install chromium
```

# Configuration

The tests run on this machine with the fake Moblin without a configuration. For PayPal, Stripe, Twitch
or a real Moblin device, copy `tests/config.example.toml` to `tests/config.toml` and modify it
to match your test setup. `tests/config.toml` is used if it exists, otherwise
`$XDG_CONFIG_HOME/moblin-website/tests/config.toml`.

Without `[paypal]` the PayPal payment tests are skipped, without `[stripe]` the Stripe ones, without `[twitch]` the Twitch lookups, and
without `--device` the tests needing a real Moblin.

## PayPal sandbox

In https://developer.paypal.com, create a sandbox REST app for a partner account and another
business (seller) sandbox account. Once per run the tests connect the seller in the dashboard
through PayPal's seller onboarding, in a Chromium whose profile is kept in `tests/cache/browser`.
If automating PayPal's pages fails, run with `--interactive` and do it by hand in the shown
browser. Viewers pay with a sandbox test card through PayPal's API, which approves orders like
PayPal's popup does, so no buyer account or browser is needed for payments.

## Stripe test mode

In https://dashboard.stripe.com, enable Connect for the platform and take the test mode secret
and publishable keys. The tests refuse live keys. The backend creates a new Stripe test
connected account for the streamer in every run, which is set up by hand in Stripe's onboarding
in the browser with `--interactive`, using Stripe's test values (the tests print them).
Without `--interactive` the tests needing it are skipped, the rest of Stripe is still tested.
Viewers pay with the test card `pm_card_visa` through Stripe's API, which confirms payments like
Stripe's Payment Element does. The payment tests run once with PayPal and once with Stripe,
the Stripe ones named with a `Stripe` suffix.

# Real Moblin device configuration

Moblin's remote control streamer connects to the tester at `remote-control-port`. The tester
acts as Moblin's assistant and sets Moblin up by importing settings. When a test gives Moblin a
Moblin Cloud key from the dashboard, Moblin Cloud connects to the tester at the same port too, and
the tester relays that connection to the backend while watching what they send each other. That
is how the tests see the songs, tips and wheel spins the backend delivers.

This is only needed for `--device`.

1. Build Moblin in debug with `MOBLIN_WEBSITE_API_URL` set to
   `http://<tester-ip-address>:<remote-control-port>` in `Config/User.xcconfig`, so Moblin
   cloud connects to the tester.
2. Generate settings into clipboard.
   ```bash
   just system-test-generate-device-settings-clipboard
   ```
3. Import the generated settings from clipboard into Moblin.
4. Allow Moblin to access Apple Music and give the device the `apple-music` capability, for the
   song request tests.

# Run the tests

```bash
just system-test
just system-test Tip
just system-test --device iphone16pro
just system-test --device iphone16pro --interactive
```

`--interactive` also runs the tests needing a person, like going live in a Moblin debug build
whose `MOBLIN_WEBSITE_API_URL` points at the backend under test instead of the tester (with
`allow-unattested = false`).
