# Backend

One binary, `src/main.rs`. The routes and who may call them are in `src/api/AGENTS.md`.

Run a single test: `cargo test payments::tests::orders_survive_restarts`. Tests are inline `#[cfg(test)]` modules per file. The API's share `start_api_with`, the socket helpers and the fake PayPal, Stripe, Twitch and Kick servers in `src/api/test_utils.rs` and `src/api/test_utils/`.

## General

- It stops on SIGTERM or Ctrl-C by finishing the requests in flight first, so a deploy never cuts a payment capture off between taking the money and saving the delivery. Streams that never finish by themselves end when the shutdown starts (`Api::stopping`).
- It does not start without `--backend-url` and `--frontend-url`, which have no defaults so a copy never points at cloud.moblin.app by mistake.
- Everything optional degrades rather than fails: without a platform's credentials its sign in answers 503.
- `--allow-unattested` disables App Attest for Moblin Cloud. `--twitch-sign-in-url`, `--kick-sign-in-url`, `--paypal-url` and `--stripe-url` are for local development and the system tests only.
- SQLite in `database.rs` is shared by `Accounts`, `Pages`, `Orders`, `Deliveries` and `Counters` behind one mutex. `counters::bump` takes a connection, for callers already holding the lock.
- Requests and the remote control traffic are counted in memory only, so a restart starts over.

## Counters

- Measurement points come in pairs: `<name>` for successes and `<name>Failed:<reason>` for failures, where the reason is the `reason` slug every `Failure` carries (`counted` does the bumping). A new reason needs adding to the admin page; see `frontend/AGENTS.md`.
- Per provider counters count from the moment the provider is asked, so the page's own refusals only count under the kind.
- A captured payment is counted for its kind and provider in the same transaction as its delivery, so it is counted once however it was found.

## Payments

Order stages only move forward: `created`, `capturing`, `captured`.

- `capturing` means the provider has been asked at least once, so the money may have been taken and only the provider can tell. An order the provider refused or the viewer had not approved stays `capturing`, like one the provider reviews.
- The stage moves to `capturing` before the provider is asked, so a database that cannot be written refuses the capture before any money moves.
- `captured` is saved together with its delivery in one transaction (`record_capture`). Two captures at once are harmless: the provider answers the second one already captured and a capture is saved once.
- A lost provider answer (a transport error, a 5xx or an unreadable body, `CaptureError::Unknown`) is looked up at the provider at once. The page retries network errors and 5xx answers, so a payment that went through never shows an error. A second capture can't take the money twice.
- A capturing order the provider has not captured stays capturing, since a lost capture may still complete after the lookup, until `settle_orders` removes it 30 minutes after creation. A pending review that completes days later still delivers.
- An order has 30 minutes to be paid, since a tip that late is no use on stream. That deadline, the settle, delivery retry and stream recheck intervals and the wait for Moblin to answer a control have hidden options (`--payment-deadline`, `--settle-interval`, `--delivery-retry-interval`, `--access-recheck-interval`, `--moblin-answer-timeout`, in seconds) only for the system tests to shorten, so production never sets them. Money a lookup finds taken past that is refunded instead of delivered (`refund_late`). A refund the provider refuses, from a streamer who connected PayPal before the `REFUND` feature was asked for, delivers late instead, and one whose answer was lost is retried every minute.
- Stripe payments the backend gives up on are cancelled, so the hold on the viewer's card is released.
- Spins need Moblin's wheel both to create an order and to capture it. Tips need no connected streamer.
- Tip and spin orders share `capture_order`, which refuses an order of another kind.

PayPal:

- The merchant ID only ever comes from PayPal's seller onboarding; the return URL's parameters are never trusted.
- Orders name the merchant ID as payee, never the email: PayPal no longer accepts emails as the SDK's `merchant-id`, and it would be public.
- Orders ask for `IMMEDIATE_PAYMENT_REQUIRED`, so funding that settles later is not offered and fewer captures end pending.

Stripe:

- Direct charges on the connected account (`Stripe-Account` header) with `capture_method=manual`, captured by the backend like a PayPal order.
- The country is picked by the streamer, since Stripe can't change it later.
- There are no Stripe webhooks; a revoked account only fails new payments until the streamer checks again.

## Deliveries

- At least once delivery to Moblin, retried until Moblin answers `ok`; Moblin's dedup on `requestId` makes the retries safe. A resend needs a new `requestId` (`<order id>-<resends>`) or the dedup swallows it.
- Spins go to the wheel device and tips and effects to every device the page knows, so one briefly disconnected still gets it when it comes back.
- A delivery is only given up after 7 days, the one way a paid request is allowed to fail.

## Moblin sessions

- `remote_control.rs` serde types mirror Swift's synthesized `Codable` (`{"ping":{}}`, optional fields omitted).
- Moblin never sends a close frame, as NWWebSocket's `disconnect()` only cancels the connection. So every disconnect of today's Moblin is kept `reconnecting` for a minute: it keeps its place and wheel, gets no requests and counts towards the 5 devices. A close of code 1000 ends the session at once.
- A device is told apart by the `streamerId` of `identify`, Moblin's per install id. One without it and a sixth device of a page are answered `identified` with an `error` result, so Moblin keeps retrying, before the socket is closed.
- Moblin pushes status only while a page is open: the first watcher sends `startStatus` and the last one's guard `stopStatus`. The periodic event only carries `topRight`, so `poll_status` sends `getStatus` every 5 s for the rest.
- A `Monitor` outlives the device's sessions, so watchers keep it when the device reconnects, and a device connecting while watched is sent the requests again (`resume_watching`). The preview works the same way apart from the status monitors.
- Older Moblins answer empty settings, push nothing and ignore the controls, and their settings have no `moblinCloud`.
- A new Store icon in Moblin needs a row in `icons.rs`, or the backend ignores it. The fart sounds in `src/effects/` are copied from Moblin's `Alerts.bundle`.

## App Attest

- `app_attest.rs` is a copy of the Moblin website's. It is stateless by design: the app resends its attestation every time and the chain is verified each time, deliberately ignoring certificate validity dates.
- The assertion's client data is the raw bytes of the hello `challenge` of this very connection, so no separate challenge round trip is needed.
- A malformed `appAttest` is read as an empty one, so it fails as `invalid` instead of making the whole message fail to parse.
- App Attest is not supported on Mac, so Moblin Cloud is off in Moblin there.

## Sign in and accounts

- Session, viewer (`viewer:`) and link (`link:`) tokens are signed the same way and none is accepted as another. A session is only accepted while its generation matches the account's `sessions` counter.
- `signed_in` also accepts a session whose account id is a Twitch user id, from before the ids were renamed, so renaming signed nobody out.
- The sign in state must match the `moblin_sign_in_state` cookie, so a callback URL only works in the browser that started it.
- A link's callback touches no account: the Account page posts the token with its session, so a callback link handed to someone else links their own identity to their own account. Nothing is ever merged: an identity of another account is refused.
- mobtop's code from `port` and `challenge` is traded for the session with the verifier, so neither another program that gets the code nor a website that started the sign in can use it.
- The Connect `url` never has the code, so a link someone sends can't fill it in.
- An account's last identity can't be unlinked, nor one that would leave only unconfigured platforms, since those can't sign in.
- The Moblin Cloud key is only used by Moblin's WebSocket and only leaves the backend in the signed in account. The page id is public, unrelated to the key, and stays when the key is replaced.
- Starting to connect PayPal needs a session issued in the last 10 minutes.
- Deleting an account never touches PayPal or Stripe accounts.

## Subscribers

- Spins without a payment are only those of Twitch and Kick subscribers and the streamer's and assistants' own tests.
- They need a linked identity on a configured platform, whatever the settings say (`linked_tiers`).
- Viewer sign in creates no account. The token lasts 24 hours, since the tier is a snapshot.
- Allowances are keyed by `Viewer::key` so the two platforms' numeric ids never collide, and are counted in one transaction per UTC month.
- Kick's check is an unofficial website API. A failed check grants nothing and tells the viewer, rather than saying they are not subscribed.

## Pages and images

- The currency only changes while tips, the wheel and effects are all off, so their amounts never silently switch currency.
- Each `Feature` has `enabled` next to its values, so turning one off keeps them. It defaults to true, so settings saved before the flag are on.
- Only premium accounts upload a banner and pick a font or custom color. Taking a streamer off `--premium-accounts` keeps what they have and only stops new choices.
- `banner` re-encodes every upload, so nothing but pixels is kept.
