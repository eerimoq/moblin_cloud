# backend subscribers

[Architecture index](../architecture.md). Keep this note current with its implementation.

`subscribers.rs` — wheel spins and effects for Twitch and Kick subscribers without a
payment, on with the page settings `subSpins` and `subEffects` (`pages::Tiers`, 0 to 100
a month per tier with at least one above 0; they need no PayPal or Stripe, and work with
or without the paid wheel and effects). Effects work like the spins described here, with
`GET`/`POST /pages/{id}/effects/subscriber` (`effect` and `message`, no wheel needed),
their own allowance (kind `effect`, `Perk` in `api/subscriber.rs`), `no-effects-left`
and the counter `subEffects`. It needs the page's account to have a Twitch or Kick
identity linked on a platform the backend is configured for: without one the public
answer shows it as null and the checks treat it as off (`linked_tiers`), whatever the
settings say, and a viewer sign in with a platform the page has not linked answers 404;
the public page handlers, the subscriber routes and the order routes read the page's
account and settings together with `Accounts::page` (`page_settings` in
`api/public.rs`), one query for the account, its identities, the settings, the banner
image version and Moblin's icon (`pages::Look`, which `Pages::look` answers alone). A
viewer signs in with `GET /auth/twitch/login?page=<id>` (scope
`user:read:subscriptions`, the page kept with the state) or `GET
/auth/kick/login?page=<id>` (the normal Kick scopes); the Twitch callback asks Helix
`subscriptions/user` for their tier to the page's streamer
(`Auth::twitch_subscription_tier`, the Twitch user id of the page's account's Twitch
identity), and the Kick callback asks Kick's website API `GET
https://kick.com/api/v2/channels/{streamer slug}/users/{viewer slug}`
(`Auth::kick_subscriber`, `KickApp::channel_users_url`, an unofficial API; the slugs are
the identities' logins, the viewer's from the official channels API at sign in) for an
active `subscriber` badge, which gives tier 1, so Kick subscribers get the tier 1
allowance. Neither creates an account; both redirect to the page under the login of the
viewer's platform (`/twitch/<login>` or `/kick/<slug>`, falling back to any linked one)
with `#viewer=<token>`, `#viewerSignInFailed=<platform>` when the sign in failed, or
`#viewerCheckFailed` when the Kick badge check failed (a transport error or an error
status), in which case the page says "Couldn't check your Kick subscription, try again."
and nothing is granted. The viewer token (`Auth::viewer_session`, `viewer_of`) is HMAC
signed like a session but prefixed `viewer:` so neither is accepted as the other, holds
the `platform`, user id, name, page and tier, and lasts 24 hours since the tier is a
snapshot; a token whose platform the page no longer has linked is refused with 401
(`viewer`). Allowances are keyed by `Viewer::key`, the Twitch user id or `kick:<Kick
user id>`, so the two platforms' numeric ids never collide. `GET
/pages/{id}/spin/subscriber` answers the viewer's `platform`, `name`, `tier`,
`allowance` and `left`. `POST /pages/{id}/spin/subscriber` (`message`) spins once and
needs Moblin's wheel like a paid spin (`wheel_available`); it (`subscriber`, then
`queue_for_subscriber`) counts in one transaction (`Allowances::take`) in the
`subscriber_allowances` table (per page, viewer key, kind and UTC month, so it resets on
the 1st; past months are pruned), save a captured order with `Payee::Twitch(tier)` or
`Payee::Kick` and price 0 (so it shows in `GET /account/recent` with its `tier`, or
`kickSubscriber`) and queue its delivery like a paid one, answering 202 with `left`.
Lowering a tier mid-month leaves 0, raising it gives the extra at once. Counted as
`subSpins`. With the fake Twitch the tier is the trailing digit of the fake login
(`sub1` to `sub3`, otherwise not subscribed); with the fake Kick a login ending in 1 to
3 is a subscriber. The page offers a sign in button per linked platform (`SubscriberBox`
in `pages/Page.tsx`, from the public page's `twitch` and `kick`), and the dashboard's
subscriber cards show only the tier 1 field, as "Kick subscribers", when only Kick is
linked.
