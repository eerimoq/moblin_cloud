# backend auth and accounts

[Architecture index](../architecture.md). Keep this note current with its implementation.

`auth.rs` — streamer sign in with Twitch (OAuth authorization code, `OAuthApp`) or Kick
(OAuth authorization code with PKCE S256, `KickApp`: authorize at
`https://id.kick.com/oauth/authorize`, token at `https://id.kick.com/oauth/token` with
the `code_verifier` kept with the state, scopes `user:read channel:read`, the user from
`GET https://api.kick.com/public/v1/users` and the login from the channel `slug` of `GET
https://api.kick.com/public/v1/channels`, both answered for the token's own user; the
Kick user id is a number, read as text; `--kick-client-id` and `--kick-client-secret`,
env `KICK_CLIENT_ID` and `KICK_CLIENT_SECRET`, without which `GET /auth/kick/login`
answers 503), each callback giving an `Identity` (platform, user id, lowercase login,
name, avatar), with one-time states (`SignInState`: the viewer's page, whether it is a
link and the PKCE verifier) that last 10 minutes and must match the
`moblin_sign_in_state` cookie set by `GET /auth/<platform>/login`, so a callback URL
only works in the browser that started the sign in. `signed_in_redirect` in
`api/sign_in.rs` handles both platforms: a normal sign in finds the account of the
identity (`Accounts::signed_in`) or creates one with a random id and a new page, and
redirects to `/dashboard/#session=<token>`; `?port=<port>&challenge=<challenge>` on the
login route (mobtop's; both or neither, the challenge 43 base64url characters, refused
with 400 otherwise and together with `page` or `link`, `Terminal`) redirects a normal
sign in to `http://127.0.0.1:<port>/?code=<code>` instead, and its failures after the
state was checked to `?signInFailed`; the code (`Auth::terminal_code`, in memory) is
single use, lasts a minute and is traded for the session by `POST /auth/session` with
`code` and `verifier`, whose base64url SHA-256 must be the challenge (`session_of_code`;
400 `Signing in failed. Try again.` otherwise, which uses the code up too), so the
session never appears in a URL, the browser's history or a log, and neither another
program on the machine that gets the code nor a website that started the sign in (not
knowing mobtop's verifier) can use it; `?link=1` on the login route makes a link flow,
whose callback does not touch any account but redirects to `/account/#link=<link token>`
(`#linkFailed` when it failed), a signed token prefixed `link:` holding the identity for
10 minutes (`Auth::link_token`, `link_of`), which the Account page posts to `POST
/account/link` with its session, so the identity is linked to the account whose browser
finished the flow and a callback link handed to someone else links their own identity to
their own account; signing mobtop in on another device is an OAuth device authorization
grant (RFC 8628) of Moblin Cloud's own, the same for Twitch and Kick: `POST
/auth/connect` answers a `deviceCode` (32 random bytes), a `userCode` (8 letters of
`BCDFGHJKLMNPQRSTVWXZ`, shown as `XXXX-XXXX`), the Connect page's `url` from
`--frontend-url` (never with the code, so a link someone sends can't fill it in and the
streamer types the code their own terminal shows), `interval` 5 and `expiresIn` 600
(`Auth::start_connect`, in memory, at most 10000, 503 past it); the signed in streamer
approves or denies it on the Connect page with `POST /account/connect` (`code`,
normalized by `user_code_of`, and `approve`; approving gives it a new session of their
account, so signing out everywhere ends it; 404 `There is no such code. Check it, or
start signing in again in mobtop.` for an unknown, expired or decided one; 429 past 10
tries a minute per account, `Api::connect_tries`), and `POST /auth/connect/session`
(`deviceCode`) answers 202 while pending, the `session` once approved, 403 `Signing in
was denied.` and 410 `The code expired. Start signing in again.`, the decided ones only
once (`connect_session`); `POST /account/unlink/{platform}` removes a linked identity,
409 for the last one and for one that would leave only identities of unconfigured
platforms (`Auth::has`), since those can't sign in; `GET /config` answers what the
server has credentials for, public and only ever booleans: `signIn` (`twitch`, `kick`)
and `payments` (`paypal`, `stripe`). Sessions are `<account id>:<generation>:<issued>`
signed with HMAC-SHA256 under `--session-secret` (random without it), 30 days, sent as a
bearer token (`signed_in` in `api.rs` also accepts a session whose account id is a
Twitch user id, from before the ids were renamed, by looking the account up through its
Twitch identity, so renaming signed nobody out); viewer (`viewer:`) and link (`link:`)
tokens are signed the same way and none is accepted as another. A session is only
accepted while its generation matches the account's `sessions` counter, which signing
out increments; starting to connect PayPal (`POST /account/paypal/connect`) needs a
session issued in the last 10 minutes. The Twitch and Kick URLs are fields of `OAuthApp`
and `KickApp` so tests can point them at a local server.

`accounts.rs` — one account per streamer, keyed by a random id of 32 hex characters
(`Streamers::new_account_id`), with its linked identities in the `identities` table
(`Identity`: `platform`, `twitch` or `kick`, `user_id`, `account`, `login`, `name`,
`avatar`; primary key platform and user id, so a platform account belongs to one Moblin
Cloud account), each giving the page an address, `/twitch/<login>` or `/kick/<login>`
(`page_of_login` per platform; a login is unique per platform: it is taken from other
identities on sign in, after renames, which leaves theirs empty). `signed_in` finds the
identity's account or creates one (the account row with its key, its page's settings row
and the identity in one transaction) and answers the id with the account; `link` adds an
identity to an account and refuses one that belongs to another account (`IdentityTaken`,
409 "This Kick account already has its own page. Delete it first."; nothing is ever
merged); `unlink` removes one and refuses the last (`LastIdentity`, 409). The account's
name and avatar (`Account::name`, `avatar`, for the dashboard header, test sends and the
Stripe account name) are those of its Twitch identity when it has one, else its Kick
one; the pages show the identity of their address. Its page: a public id fixed at sign
in, its Moblin Cloud key (`moblin_key`, uniquely indexed so the WebSocket route's
`page_of_key` is an indexed lookup; `POST /account/key` replaces it and the WebSocket
route only accepts the current key), and its PayPal: the merchant ID every paid feature
of the page pays (`payment_targets` in `api/settings.rs`, the `payee` of an order) and
the tracking ID of an unfinished onboarding, and its Stripe connected account with
whether it takes payments yet (only ready accounts). The merchant ID only ever comes
from PayPal's seller onboarding (Partner Referrals): `POST /account/paypal/connect`
creates a referral and answers PayPal's sign up link, and `POST /account/paypal/check`
looks the tracking ID up with PayPal and connects the merchant ID once it can receive
payments, has a confirmed email and granted our app third-party permissions; the return
URL's parameters are never trusted. `POST /paypal/webhooks` (verified with PayPal's
verify webhook signature API under `--paypal-webhook-id`) connects pending onboardings
on `MERCHANT.ONBOARDING.COMPLETED` through the same `finish_onboarding` checks and
disconnects merchants on `MERCHANT.PARTNER-CONSENT.REVOKED`; `POST
/account/paypal/disconnect` forgets the merchant ID and any unfinished onboarding. Paid
features can only be turned on or changed while PayPal or Stripe is connected. Saved in
the `accounts` table. `POST /account/delete` (`Accounts::delete`, the Account page, with
a confirm) removes the signed in account and everything it owns in one transaction: the
account row, its identities, its devices, its monitor layouts and those others saved for
its page, both directions of each identity's assistant rows, its page's settings, banner
image, Moblin icon, orders, deliveries and subscriber allowances; the handler then
disconnects its Moblin devices, and signing in again starts a new account with a new
page id. PayPal and Stripe accounts are never touched.
