# backend payments

[Architecture index](../architecture.md). Keep this note current with its implementation.

`paypal.rs` — PayPal partner client: seller onboarding (`referral`, `seller`, which need
the partner's merchant ID `--paypal-partner-id`; `--paypal-attribution-id` is the BN
code sent with every request), webhook verification and Orders v2, created and captured
for the streamer with an unsigned `PayPal-Auth-Assertion` JWT naming their merchant ID,
for tips and wheel spins (an order's `Item` is a tip with its message or a spin purchase
with its number of spins and message): orders name the streamer's PayPal merchant ID as
payee so money goes straight to them (never the email: PayPal no longer accepts emails
as the SDK's `merchant-id`, and it would be public), with
`payment_source.paypal.experience_context` asking for no shipping and
`IMMEDIATE_PAYMENT_REQUIRED`, so funding that settles later (eCheck) is not offered and
fewer captures end pending. A completed capture whose amount matches queues the
delivery; a pending one (PayPal still reviewing it) is settled later by `settle_orders`,
which asks `status` (GET the order).

`stripe.rs` — Stripe Connect platform client, the alternative to PayPal. Streamers get a
connected account (Accounts v2 API, full Stripe Dashboard, Stripe collects fees and
losses) created on the first `POST /account/stripe/connect` in the country the streamer
picks (`stripe::COUNTRIES`, the countries Stripe takes sign ups from, also listed in the
frontend; Stripe can't change it later), which answers a Stripe-hosted onboarding link
back to `/dashboard/?stripe`; `POST /account/stripe/check` marks it ready once its
`card_payments` capability is active. Payments are direct charges on the connected
account (`Stripe-Account` header): a PaymentIntent (v1 API) with
`capture_method=manual`, no redirect payment methods and no application fee, confirmed
in the browser with the client secret and captured by the backend, like a PayPal order.
On the page, "Pay with Stripe" creates the order first (so the viewer sees the final
price) and then shows Stripe's Payment Element; changing the form closes it. `status`
(GET the PaymentIntent) settles payments whose capture answer was lost. There are no
Stripe webhooks; a revoked account only fails new payments until the streamer checks
again. `--stripe-secret-key` and `--stripe-publishable-key`; a test mode key takes test
payments.

`payments.rs` — what is provider neutral about payments: the order (`Item`, `Order` with
its `Payee`, a PayPal merchant ID, a Stripe account, or `Twitch` with the tier or `Kick`
for a subscriber's spin, saved already captured and never reaching a provider, and its
description), `CaptureError` (`NotApproved`, `Pending`, `Refused` when the provider
definitely said no, `Unknown` when its answer was lost; an order the provider has
already captured is `Ok`), `PaymentStatus` (what a lookup at the provider says:
captured, pending, not captured or gone, or refunded after a late payment),
`RefundError` (`Refused`, `Unknown`) and `Orders`, every order in the `orders` table
with its `Stage` (`created`, `capturing`, `captured`) and the order as JSON, so a
restart loses no payment: `start_order` inserts after creating the provider's order
(refusing the order with 503 `not-saved` if that fails), `transition` is a
compare-and-set between stages, `captured` runs the stage change and the delivery insert
in one transaction, `get` reads an order with its stage and whether it is past the
deadline (`Saved`), `unsettled` lists the capturing orders older than a given age for
`settle_orders`, `expire` drops created orders after 30 minutes (`settle_orders` then
cancels their Stripe payments, so a viewer whose capture never reached the backend gets
the hold on their card released), `make_room` refuses new ones past 10000 unfinished,
`captured` does nothing for an order already captured and otherwise counts
`<kind>Payments` in the same transaction as the delivery, so a payment is counted once
however it was found, and `prune` gives unsettled orders up and forgets captured ones
after 30 days.

`deliveries.rs` — at least once delivery to Moblin of tips and wheel spins, saved in the
`deliveries` table. Deliveries are kept per page and their devices picked when
delivering: spins go to the wheel device and tips to every device the page knows
(`tip_devices`: the connected ones and those in the `devices` table, so one briefly
disconnected when the tip was ordered still gets it when it connects again), tracked in
`delivered` (by `streamerId`) so each device gets a tip once; a tip is finished once a
device got it and every known device has it, so a tip waiting for a device that never
comes back stays unfinished until it is given up after 7 days or the device is
forgotten, and a device connecting while a tip waits gets it too. A delivery every
device got is marked `finished` and kept 30 days, so the dashboard can tell which
devices got a tip. Test deliveries (`add_test`) are marked with their kind in `test`,
and only the 5 newest per page and kind are kept. A background task delivers them
whenever a device connects, after a request and every 10 s, reading each page's known
devices once per pass, each connected device concurrently, by sending `tip` with the
request id (random, or the PayPal order id) as `requestId` until Moblin answers `ok`;
Moblin's own dedup on `requestId` makes the retries safe. A Moblin `error` is temporary
and retried; `failure` records the latest reason so each failure is counted once per
delivery and reason. A delivery waits in the table until a device gets it and is only
given up after 7 days (the one way a paid request is allowed to fail), counted as
`tipExpiries` or `spinExpiries` when no device got it.

Everything optional degrades rather than fails: without Twitch credentials, Twitch sign
in answers 503; without Kick credentials, Kick sign in answers 503. `--allow-unattested`
disables App Attest for Moblin Cloud (so any client, the fake Moblin included, connects,
whatever `--require-attested-moblin` says), `--twitch-sign-in-url <base>` points the
sign in's OAuth and Helix endpoints (`OAuthApp::at`) at another server and
`--kick-sign-in-url <base>` Kick's (`KickApp::at`, including the website API's
`channel_users_url` the subscriber check uses), with no client ID or secret needed, both
for local development and the system tests only. There is no fake logic in the backend:
the fake Twitch is `tests/utils/fake_twitch.py` (stdlib only), an HTTP server whose
authorize page lets the browser pick any made-up login (or takes it as `login` in the
query, which the system tests do), whose user id is a 9 digit number hashed from the
login (`user_id_of`, like Twitch's numeric ids), with a login ending in 1 to 3
subscribed at that tier to everyone and any login but `nobody` resolved by its users
endpoint under an app access token (client credentials); the fake Kick is
`tests/utils/fake_kick.py`, the same for Kick's OAuth (checking the PKCE challenge, with
client credentials for the username lookup), public API (also by `slug`) and the website
API's channel user (a login ending in 1 to 3 has a subscriber badge), whose user id is
an 8 digit number hashed from the login, so the same login on both fakes is two
different users until linked. `just dev-backend` starts them on ports 8081 and 8082 and
passes both options, with `--admins` holding the user ids of `admin` on both, making it
an admin, and so premium, and `--premium-accounts` those of `premium`, making it a
premium account; `normal` is neither. The system tests start both on free ports when
`[twitch] sign-in` is `fake` (`Backend.fake_twitch`, `Backend.fake_kick`).
