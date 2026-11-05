# backend api

[Architecture index](../architecture.md). Keep this note current with its implementation.

One binary, `backend/src/main.rs`, wired together from the modules below. It stops on
SIGTERM or Ctrl-C by finishing the requests in flight first (axum's graceful shutdown),
so a deploy never cuts a payment capture off between taking the money and saving the
delivery; the device event streams, which never finish by themselves, end when the
shutdown starts (`Api::stopping`) so they do not hold it up.

`api.rs` — the axum router, `Api`, `Failure` and the session checks (`signed_in`,
`admin`, `assistant`); the handlers are in `api/` by feature (`streamers`, `sign_in`,
`account`, `settings`, `assistants`, `devices` for watching and controlling a device,
`providers` for connecting PayPal and Stripe, `public` for the public page answers,
`tips`, `spins`, `effects`, `subscriber`, `orders` for creating, capturing and settling
payments, `recent`, `banner`, `test_sends`, `monitor` for the monitor layouts, `admin`),
each with its tests. `GET /account/pages/{id}/monitor` answers the caller's monitor
`layout` for that page (the page's own account or one of its assistants, `assistant()`),
else the one they saved most recently for any page, else null; `POST
/account/pages/{id}/monitor` saves `layout`, an object at most 16 KB (413 past it, 422
`invalid-layout` otherwise) the backend stores as it is (`Accounts::monitor_layout` and
`set_monitor_layout`, the `monitor_layouts` table keyed by the account and the page).
Routes: `GET /account/pages/{id}/events`, `GET /pages/{id}/events`, `GET
/admin/stats/events`, `GET /account/pages/{id}/devices/{device}/events`, `GET
/account/pages/{id}/devices/{device}/preview`, `POST
/account/pages/{id}/devices/{device}/control`, `GET /remote-control/streamer/{key}`
(Moblin's WebSocket), `GET /pages/twitch/{login}` and `GET /pages/kick/{login}` (the
page id of a login on that platform), `GET
/pages/addresses?logins=twitch:<login>,kick:<slug>` (for the Moblin website's Streamers
list: an object keyed by those of the at most 100 comma separated logins that have a
page, each with the page's `url` on the frontend and Moblin's `icon`, the URL of its
`GET /icons/{id}` or null; the logins are looked up in one query,
`Accounts::pages_of_logins`, and the icons in another, `Pages::icons`), `GET
/pages/{id}` (the page's look, with the `twitch` and `kick` identities, null when not
linked, and Moblin's `icon`), `GET /pages/{id}/image` (its banner image), `GET
/pages/{id}/spin/subscriber`, `POST /pages/{id}/spin/subscriber`, `GET
/pages/{id}/tips`, `POST /pages/{id}/tips/orders`, `POST
/pages/{id}/tips/orders/{order_id}/capture`, `GET /pages/{id}/spin`, `POST
/pages/{id}/spin/orders`, `POST /pages/{id}/spin/orders/{order_id}/capture`, `GET
/pages/{id}/effects`, `GET /pages/{id}/effects/subscriber`, `POST
/pages/{id}/effects/subscriber`, `POST /pages/{id}/effects/orders`, `POST
/pages/{id}/effects/orders/{order_id}/capture`, `GET /effects/{id}`, `GET /icons/{id}`,
`GET /config`, `GET /auth/twitch/login`, `GET /auth/twitch/callback`, `GET
/auth/kick/login`, `GET /auth/kick/callback`, `POST /auth/session`, `POST
/auth/connect`, `POST /auth/connect/session`, `GET /account`, `POST /account/link`,
`POST /account/connect`, `POST /account/unlink/{platform}`, `POST /account/key`, `POST
/account/devices/{device}/name`, `POST /account/devices/{device}/remove`, `POST
/account/settings`, `POST /account/image`, `POST /account/image/remove`, `POST
/account/paypal/connect`, `POST /account/paypal/check`, `POST
/account/paypal/disconnect`, `POST /account/stripe/connect`, `POST
/account/stripe/check`, `POST /account/stripe/disconnect`, `POST /account/sign-out`,
`POST /account/delete`, `GET /account/recent`, `POST /account/recent/{order}/resend`,
`POST /account/test/tip`, `POST /account/test/spin`, `POST /account/test/effect`, `POST
/account/assistants/{platform}`, `POST
/account/assistants/{platform}/{assistant}/remove`, `POST
/account/assistants/{platform}/{assistant}/preview`, `POST
/account/assistants/{platform}/{assistant}/go-live`, `POST
/account/assistants/{platform}/{assistant}/edit-tips`, `POST
/account/assistants/{platform}/{assistant}/remote-control`, `GET /account/pages/{id}`,
`POST /account/pages/{id}/settings`, `POST /account/pages/{id}/image`, `POST
/account/pages/{id}/image/remove`, `POST /account/pages/{id}/test/tip`, `POST
/account/pages/{id}/test/spin`, `POST /account/pages/{id}/test/effect`, `GET
/account/pages/{id}/recent`, `POST /account/pages/{id}/recent/{order}/resend`, `GET
/account/pages/{id}/monitor`, `POST /account/pages/{id}/monitor`, `POST
/paypal/webhooks`, `GET /admin/stats`. The WebSocket, Twitch and Kick callbacks and
PayPal webhook paths are registered outside this repo (in Moblin, Twitch's, Kick's and
PayPal's consoles) and must not be renamed, and `GET /pages/addresses` is a contract
with the Moblin website's backend. Every account has a page, created with its Moblin
Cloud key in the sign in callback of the platform that created it, and managed by the
signed in account, whichever linked platform it signed in with. An account has one
Moblin Cloud key (a random lowercase version 4 UUID from `Streamers::new_key`; `key` in
the signed in account, replaced with `POST /account/key`, which disconnects every
device), only used by Moblin's WebSocket and only leaving the backend in the signed in
account, which the streamer pastes into the global Moblin Cloud settings of up to 5
devices running Moblin; Moblin connects to `/remote-control/streamer/{key}` under its
`MOBLIN_CLOUD_API_URL`, separately from its own remote control. The page's public id is
12 random bytes in base64url (`Streamers::new_page_id`), unrelated to the key, and stays
when the key is replaced. A device is told apart by the `streamerId` of its `identify`
message (Moblin's per install id): one without it is refused, as is a sixth device of a
page, both answered `identified` with an `error` result (Moblin shows "Failed to
identify" and keeps retrying) before the socket is closed (`remote_control::refuse`),
while a device connecting again replaces its old session in its place, keeping its wheel
until Moblin pushes it again. A session whose connection ended without a close frame of
code 1000, normal closure, (a transport error, a failed send, 60 s without a message or
any other close; Moblin never sends a close frame, as NWWebSocket's `disconnect()` only
cancels the connection, so this is every disconnect of today's Moblin, turning Moblin
Cloud off included) is kept `reconnecting` for a minute (`Streamers::new`'s linger, zero
in the tests but `sessions_wait_for_the_device_to_reconnect`) so a device with flaky
internet comes back to the same session: it keeps its place, its wheel (so the page
still takes spins and the settings' `connected` stays true) and counts towards the 5
devices, but gets no requests (`RequestError::Offline`, so deliveries wait for it to
reconnect), is not counted in `connected_count` and is shown as `reconnecting` in the
dashboard's device list; a 1000 close ends the session at once (the system tests' relay
in `tests/utils/moblin.py` sends it when the tester changes Moblin's settings, and 1001
otherwise, so its `disconnect()` acts like a dropped connection). Remote control
sessions are per page and device, in the order they connected: wheel spins use the
page's wheel device, the first with a wheel picked there (else the first one), and tips
and effects go to every known device of the page (`Deliveries::tip_devices`), each as it
connects; the settings' `connected` is whether any device is. The signed in account's
`devices` lists the page's known devices, the connected ones first in the order they
connected and then the others, seen most recently first, each with its `name` (`Moblin
<number>` with the smallest number no other device of the page is named with, given when
the page first sees it, or the name the streamer gave it with `POST
/account/devices/{device}/name`, 1 to 30 characters; `Accounts::seen_device` and
`rename_device` in the `devices` table, keyed by the account and the `streamerId`, its
`last_seen` updated when it connects and when it disconnects (`Accounts::left_device`),
forgotten with its name after 30 days unseen or when the streamer removes it with `POST
/account/devices/{device}/remove` (`Accounts::remove_device`, 409 `Turn off Moblin Cloud
on the device first.` while it is connected, since it would come back at once; tips then
stop waiting for it), and at most 20 kept per account, a new one replacing the one seen
longest ago), whether it is `connected` or `reconnecting`, whether it is the `wheel`
device, when not connected, when it was last seen (`lastSeen`, unix seconds), and its
`watchers`, the `name` and `avatar` of each assistant with an event stream of it open
(`Streamers::watchers`, the accounts each device's `Monitor` counts, read in one
`Accounts::get_many`, the streamer's own left out; empty in an assistant's `GET
/account/pages/{id}`), which the dashboard's Moblin section shows, the watchers as
Watching now under the device. Spins without a payment are only those of Twitch and Kick
subscribers (`subscribers.rs`) and the streamer's and assistants' own tests. Tips need
no connected streamer; a completed tip capture queues the tip (`deliveries.rs`) for
Moblin. Wheel spins need Moblin's wheel (`wheel`, so a connected streamer that sent one)
both to create an order and to capture it; a completed spin capture queues the spins the
same way. Order requests take an optional `provider` (`paypal`, the default, or
`stripe`), the public tip and spin answers carry the PayPal fields and `stripe`
(publishable key and connected account) for whichever the streamer connected, and a
Stripe order also answers its `clientSecret`. Tip and spin orders share `capture_order`,
which refuses an order of another kind (`Item::kind`), and cancels a Stripe payment it
refuses because the wheel is gone, so the hold on the viewer's card is released. A
capture moves the order through the stages in `payments.rs`: `created` to `capturing`
before the provider is asked (a database that cannot be written refuses the capture with
503 `not-saved` before any money moves; two captures at once are harmless, since the
provider answers the second one already captured and `record_capture` saves a capture
once), then `captured` together with its delivery in one transaction (`record_capture`);
the stages only move forward, so an order the provider refused (402 `declined`, a 4xx so
the page does not retry it) or the viewer had not approved (402 `not-approved`) stays
`capturing` like one the provider reviews, and `capturing` means the provider has been
asked at least once, so the money may have been taken and only the provider can tell.
When the provider's answer is lost (a transport error, a 5xx or an unreadable body,
`CaptureError::Unknown`), the capture handler looks the order up at the provider at once
(`settle_one`) and only answers 502 `unknown` ("did not answer. If the money was taken,
the tip will still be shown.") when the lookup fails too; a retried capture of a
capturing order looks it up again and, when the provider has not captured it, runs the
same checks as a first capture (the wheel for spins) and captures it again (a second
capture can't take the money twice), and a captured one answers 202 again; a capturing
order the provider has not captured stays capturing, since a lost capture may still
complete after the lookup, until `settle_orders` finds it still not captured 30 minutes
after it was created and removes it (cancelling a Stripe payment), so the page retries a
capture on network errors and 5xx answers (three times) without ever showing an error
for a payment that went through. An order has 30 minutes from its creation to be paid
(`Saved::late`, `DEADLINE` in `payments.rs`), since a tip that late is no use on stream:
a capture of a created order past it is refused with 410 `too-late` before the provider
is asked, and money a lookup finds taken past it (a lost capture answer or a pending
review completing late) is refunded instead of delivered (`refund_late`:
`PayPal::refund`, the order's capture refunded with `PayPal-Request-Id` `refund-<capture
id>`, which needs the `REFUND` feature the referral asks for, and `Stripe::refund`,
`POST /v1/refunds` with `Idempotency-Key` `refund-<payment id>`), the order removed and
counted as `<kind>Refunds` and `<provider>Refunds`, with 410 `refunded` to a viewer who
retries; a refund the provider refuses (a streamer who connected PayPal before the
`REFUND` feature was asked for) delivers the payment late instead
(`<provider>RefundsFailed:provider`), and one whose answer was lost is retried every
minute (`:unknown`). `settle_orders` is a background task that, every minute, looks up
every capturing order older than a minute (`PayPal::status`, `Stripe::status`) and
queues the delivery of those the provider captured, so a pending review that completes
days later, a capture whose answer was lost or a crash between the provider's answer and
the database still deliver. The `/account/test/*` routes let the signed in streamer send
themselves a free tip or wheel spin from the dashboard's Tips and Wheel of luck
sections, to check that Moblin gets them: they skip payment and need no PayPal or
Stripe, but run the same Moblin checks as paid ones (the wheel for spins, nothing for
tips, which wait in the queue for Moblin like paid tips), so spins are refused with 409
`Connect Moblin first.` without a connected device; all are refused with 429 past 10
tests a minute per page (`Api::tests`, in memory), and queue a delivery under the
account's name with the first suggested tip amount (5.00 without tips) or the first spin
option's spins (1 without a wheel option). `GET /account/recent?kind=<tip|spin|effect>`
pages through the signed in streamer's captured orders of that kind, newest first
(`Orders::recent`, by the `kind` column and the `orders_recent` index; captured orders
are kept 30 days, so that is the whole history), `limit` of them (20 by default, at most
50) `before` the order id given, answering `items` and whether there are `more`; without
`kind` it answers the newest `limit` of every kind in one request, as `tip`, `spin` and
`effect`, each with its `items` and `more` (400 with `before`), which the dashboard, the
Activity panel and mobtop fetch when they open and on every `recent` event with the
largest shown count, paging per kind only for Show more (`Orders::recent_of_every_kind`,
one query numbering the orders per kind with a window function, and one
`Deliveries::get_many` for all of them); test deliveries are not listed. A paid item has
its `provider` (`paypal` or `stripe`), only in the streamer's own list, never in an
assistant's (`GET /account/pages/{id}/recent`). Each item has its `status`: `delivered`
when its delivery is finished (or no delivery row is left), `waiting` while Moblin has
not got it, and its `devices` with whether each `received` it (a finished delivery lists
only the devices that got it, an unfinished one also the ones it still waits for: every
connected and known device for a tip and the wheel device for a spin), which the
dashboard shows in place of a status, except `waiting` with no device listed ("Waiting
for Moblin"). The dashboard shows each kind under its section's test card (`Latest` and
`createRecent` in `components/Latest.tsx`): the first 5 when the dashboard opens, then
20 more per Show more, refreshed (the shown ones, at most 50; older loaded ones are
kept) on the page events' `recent`, each item with a Send again button (`POST
/account/recent/{order}/resend`, or `/account/pages/{id}/recent/{order}/resend` for an
assistant) that queues the captured order's delivery again (`Deliveries::resend`: the
delivery row is reset, or inserted when it was pruned, with `resends` bumped, so Moblin
gets it under a new `requestId` of `<order id>-<resends>` and its dedup does not swallow
it; it then shows as `waiting` until Moblin gets it). Viewer-facing errors are full
sentences the page shows as they are. `GET /admin/stats` is for the site operator: the
`twitch:<Twitch user id>` or `kick:<Kick user id>` in `--admins` (env `ADMINS`, comma
separated; a bare id is a Twitch one and any other prefix is refused at start,
`accounts::admin_key_of`) are admins (an account is an admin when any of its identities
is listed, `Identity::admin_key`), `GET /account` answers `admin` for them and everyone
else gets 403; it answers counts of accounts (total, PayPal and Stripe connected, and
premium: signed in admins and `--premium-accounts`), connected devices
(`connected_count`, every session of every page) and the remote control `messages` sent
to and received from Moblin's WebSocket connections (`traffic.rs`, `Streamers::traffic`:
the `messages` and `bytes` per `kind`, the request or event name, `hello`, `identified`,
`pong`, `ping`, `identify` or `response:<result>`, with an unreadable message as
`invalid` and kinds past the first 100 as `other`; in memory only like the requests,
shown as two tables in the admin page's Moblin section), unfinished orders and pending
deliveries that no device has received yet (a tip some of the page's devices got is not
pending; plain column counts, no JSON is read), the SQLite database size (`database`:
the total `bytes` and the `files` on disk, the main, `-wal` and `-shm` ones that exist,
each with its `name` and `bytes`, none in memory; the admin page's Server section lists
them with the total), the `server`: its `version` (the release tag, passed to the Docker
build as `VERSION` by the deploy workflow and compiled in as `MOBLIN_CLOUD_VERSION`,
`dev` otherwise), when it `started` and the server's `now` (unix seconds, for the
uptime), its resident `memory` (from `/proc/self/status`, null off Linux), which
`integrations` are configured (Twitch, Kick, App Attest, PayPal, Stripe) and its
`requests` (`requests.rs`: the `total` HTTP requests answered since it started, the
count per `statuses` code and the `endpoints`, each matched route pattern like
`/pages/{id}/tips` (axum's `MatchedPath`, `*` for requests matching no route) with its
own `total` and `statuses`, most requested first; from a
`middleware::from_fn_with_state` layer outside the CORS layer that counts each response
in a map of the pattern's status counts behind a mutex, in memory only, so a restart
starts over; the admin page's Server section shows them as tiles, a status table and an
endpoint table with 4xx and 5xx columns, with 5xx counted as Server errors), plus the
event `counters` (`counters.rs`); unfinished orders are those not yet captured. The
change streams (`api/events.rs`) are fed by `Changes` in `changes.rs`
(`Streamers::changes`), a broadcast channel per page that is only kept while someone
listens: `Change::Page` for anything the public page shows (settings, banner, PayPal and
Stripe connected or not, linked platforms, a device connecting, reconnecting, leaving or
pushing its wheel), `Change::Account` for what only the dashboard shows (device names
and removal, an account starting or stopping to watch a device, assistants, which also
tells the assistant's own page, an onboarding started) and `Change::Recent` for a
captured or resent order and a delivery a device got. `GET /account/pages/{id}/events`
sends `account` for both of the first two and `recent` for the last (both when it
lagged) and checks the assistant again every minute like the device stream; `GET
/pages/{id}/events` (404 for an unknown page) sends `page`; `GET /admin/stats/events`
sends `stats` at once and every 2 s while the account is still an admin. All of them end
when the shutdown starts. CORS allows only the origin of `--frontend-url` (env
`FRONTEND_URL`). The backend does not start without `--backend-url` (env `BACKEND_URL`,
its own address, from which it makes the sign in callbacks) and `--frontend-url`, which
have no defaults so a copy never points at cloud.moblin.app by mistake.
