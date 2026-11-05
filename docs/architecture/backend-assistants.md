# backend assistants

[Architecture index](../architecture.md). Keep this note current with its implementation.

Assistants (in `api/assistants.rs`, stored by `accounts.rs`) — an allow list per
platform the streamer keeps by hand in the dashboard's Assistants section, which shows a
list with a username form for each linked platform; Moblin Cloud never asks Twitch or
Kick who moderates a channel. `POST /account/assistants/{platform}` with `username` (409
`Link Twitch first.` or `Link Kick first.` without that identity, 404 `no such platform`
for any other platform) resolves the username to a user id, with Helix `GET
/helix/users?login=<username>` under an app access token (client credentials at the
token URL, `Auth::twitch_user`) for Twitch or the official `GET
https://api.kick.com/public/v1/channels?slug=<username>` under an app access token
(`Auth::kick_user`) for Kick (both through `Auth::user`; 400 `Enter a Twitch username.`
for a username that can't be one, 404 `There is no Twitch user named <username>.` when
the platform lists none, 502 `Couldn't look the Twitch user up. Try again.` when the
lookup fails, 409 `That is your own Twitch account.` for the streamer's own account, 409
`at most 50 assistants` per platform) and saves it with the login and name at the time
in the `twitch_assistants` or `kick_assistants` table (`assistants_table`; broadcaster's
and assistant's user ids on that platform, `Accounts::add_assistant`), so a rename
changes nothing about who has access; `POST
/account/assistants/{platform}/{assistant}/remove` (the user id) drops the row and takes
effect at once (404 `no such assistant` when not listed). Each assistant also has a
`preview` flag (the `preview` column, off when added, kept when added again, and off for
those listed before it existed), whether they may see the stream preview on the device
page, turned on and off per assistant with `POST
/account/assistants/{platform}/{assistant}/preview` (`on`, 404 `no such assistant`), a
checkbox in the dashboard's assistant list (each assistant's checkboxes grouped by where
they apply, Dashboard for `editTips` and Remote control for the access picker, `preview`
and `goLive`, each explained in a tooltip on hover and keyboard focus, `PermissionRow`,
`AccessPicker` and `PermissionBox`, with Preview disabled under No access and Go live
under anything but View and control), and likewise a `goLive` flag (the `go_live`
column, `POST /account/assistants/{platform}/{assistant}/go-live`), whether they may go
live from the device page (ending the stream is always allowed), and an `editTips` flag
(the `edit_tips` column, `POST /account/assistants/{platform}/{assistant}/edit-tips`,
off when added like the others), whether they may change the streamer's tip page, tips,
wheel and effect settings on the dashboard (`editor`, 403 `no-edit-tips` `The streamer
has not let you edit tips.` from the settings and banner routes; test sends and Send
again are always allowed); all three are an `accounts::Permission`, set with
`Accounts::set_assistant_permission` and read with `Accounts::rights`, which answers all
three and the access level of one assistant row in one query as an `accounts::Rights`.
Each assistant also has a `remoteControl` access level (`accounts::Access`: `none`,
`view` or `control`, stored as 0, 1 and 2 in the `remote_control` column, `none` when
added and for those listed before it existed, kept when added again), set with `POST
/account/assistants/{platform}/{assistant}/remote-control` (`access`, 422 for anything
else, 404 `no such assistant`; `Accounts::set_assistant_access`, read with `rights`
too), picked in the dashboard as No access, View only or View and control: `view` lets
them open the device page's event stream and, with `preview`, the preview (`watcher`,
403 `no-remote-control` `The streamer has not let you use the remote control.`
otherwise, also on the minute's recheck), and `control` also lets them send controls
(403 `no-control` `The streamer has not let you control Moblin.`; `sendMessage` only
needs `view`); under `none` the device page says so and the dashboard's Moblin section
hides the devices' Remote control links. All six answer the account. The tables hold
platform user ids on both sides, never account ids, and are resolved to accounts through
the `identities` table (`Accounts::assisted_by`, one query for the channels' accounts on
both platforms, and `rights`), so an account without that platform has no assistants on
it, and a channel's list survives unlinking and relinking it. `GET /account` answers
`twitchAssistants` and `kickAssistants` (`Accounts::assistants_of`, both lists in one
query, each with the `id`, its `preview`, `goLive` and `editTips`, the login and name of
their identity when they have signed in to Moblin Cloud, else the ones resolved when
added, and the `avatar` of that identity, null until they have signed in, `lastActive`,
when their account last called a route for the page, and `lastRemoteControl`, when it
last called one of the page's device routes (unix seconds, null if never), which the
dashboard's assistant list shows (`ActivityLine`, as Active now and Using the remote
control now for 3 minutes after them)) and `assisting`, the channels the user assists
(each with the `platform` they assist through and the name, login and avatar of the
channel's identity on it, plus the channel's `twitch` and `kick` identities, null when
not linked, so the dashboard opens a channel under either of its logins); an assistant
assists a page when their identity's user id on a platform is listed under the page's
account's user id on that platform (`page_user` in `api.rs` loads the page's account
once and merges the assistant's `Rights` over both platforms, the owner getting
`Rights::all`, into a `PageUser` holding the signed in account, the page's account and
the rights, which `assistant()`, `watcher()`, `previewer()` and `editor()` answer, so a
request reads the database once for the session, once for the page and once per linked
platform whatever it checks, and the handlers take the page's account and the rights
from it instead of looking them up again). `assistant()` records the time an assistant's
account calls a route for the page as `at`, and `watcher()` (the device event stream,
the preview and the controls) as both `at` and `remote_control_at` (`Accounts::active`,
the `assistant_activity` table keyed by the account and the page, written at most every
30 s per kind, each write followed by `Change::Account`; open streams call them on their
minute's recheck, so an open stream keeps them active). Deleting an account removes its
assistant rows in both directions on every linked platform. The routes take the normal
session and are allowed for the page's own account or one of its assistants
(`assistant()`, 403 `not-assistant`): `GET /account/pages/{id}/recent` answers what `GET
/account/recent` answers the streamer, and `POST
/account/pages/{id}/recent/{order}/resend` sends an item again like the streamer's. An
assistant gets the streamer's dashboard: `GET /account/pages/{id}` answers the page's
account like `GET /account` but without payments (`without_payments`: `paypal` and
`stripe` false, no merchant or Stripe account id, nothing pending), with the page's
`devices` (names and connection state, never the key) and no `assistants` or
`assisting`, `admin` false, the streamer's `premium` and `preview`, whether the caller
may see the page's stream preview (`Rights::permits(Permission::Preview)`: the page's
own account, or an assistant with `preview` on either platform; `GET /account` answers
it true) and `goLive`, whether the caller may go live (`Permission::GoLive`, likewise
true in `GET /account`; the device page disables Go Live, but not End, without it)
`editTips` (`Permission::EditTips`, likewise; without it the dashboard says so and
disables every settings card, `Editable` in `pages/Dashboard.tsx`) and `remoteControl`,
the caller's access level (`Rights::remote_control`, `control` in `GET /account`; the
device page shows a message in place of the device under `none` and disables the phone's
controls, but not the message to the streamer, under `view`); `POST
/account/pages/{id}/settings` saves the look, subscriber tiers, tips and wheel like the
streamer's `POST /account/settings` (so turning a paid feature on or changing its
amounts needs the streamer's PayPal or Stripe connected, 409 otherwise) but keeps the
page's currency whatever the request says (`handle_assistant_settings`), `POST
/account/pages/{id}/image` and `/image/remove` change the banner (premium by the
streamer, counted as `imageUploads`) and `POST /account/pages/{id}/test/tip` and
`/test/spin` send the assistant's own test tip or spin like the streamer's
`/account/test/*` (same limit and counters, under the assistant's name). `GET
/account/pages/{id}/devices/{device}/events` (`api/devices.rs`, for the owner and
assistants alike, so the device page uses it for both) is a server-sent event stream
(axum's `Sse`, keep alive comments, the bearer session in the header like every other
call since the browser reads it with `fetch`; `watcher`, the assistant check plus the
`view` access level, is run again every minute and the stream closed when it fails, so
signing out everywhere, being removed as an assistant or set to No access ends an open
stream) of the device as Moblin pushes it (`Streamers::watch`): first a `snapshot` (the
device's `name`, `connected`, the merged `status` with `general`, `topLeft` and
`topRight`, the merged `state`, the `settings` or null and, while connected, the
session's `traffic` like the `status` events', else null), then `status` (the parts
Moblin sent plus the `sample` taken from `topRight`: `at` in unix milliseconds,
`bitrate` in bits per second and `totalBytes` from `bitrateInfo`, `bonding` from
`bondingInfo` with each connection's `name`, `bytesPerSecond` and `rtt`, and the
session's `traffic`, the `sent` and `received` `messages` and `bytes` of its WebSocket
(`traffic::SocketTraffic`, held by the `remote_control::Socket` the WebSocket handler
wraps the socket in before `hello` and hands to `identify`, `refuse` and `serve`, with
`connect` keeping the counters in the `Session`; `send` and `receive` count every
message there next to the admin's `Streamers::traffic`, so a new connection starts over;
shown as the last row of the Monitor's Status panel, `↑ <bytes> (<messages>) ↓ <bytes>
(<messages>)` from Moblin's point of view like the rest of the panel (↑ what Moblin
sent, the socket's `received`), with the full numbers in its tooltip on the web)),
`state` (`data`, the fields that changed), `settings` (`data`) and `connection`
(`connected`) events; 404 `This Moblin is not in the list.` for a device the page does
not know; a known device that is not connected gives a stream too, with `connected`
false in the snapshot and `connection` true once it connects, so an open device page
never gets errors while waiting for its Moblin. `POST
/account/pages/{id}/devices/{device}/control` takes one of Moblin's requests as the body
(`remote_control::Control`: `setRecord`, `setLive`, `setMute`, `setStealthMode`,
`setTorch`, `setZoom`, `setZoomPreset`, `setScene`, `setBitratePreset`, `setMic`,
`setSrtConnectionPriority`, `setSrtConnectionPrioritiesEnabled`, `reloadBrowserWidgets`,
`instantReplay`, `setAutoSceneSwitcher` (no `id` for off), `setPreviewStream`,
`setFilter` (`filter` like `{"grayScale":{}}`), `startMacro`, `stopMacro`,
`setGimbalTracking`, `sendMessage` (`text`, 1 to 300 characters, `Control::valid`, and
`name`, set to the caller's account name), `setMoblinCloudAlertsWidget` and
`setMoblinCloudWheelOfLuckWidget` (an optional `id`, none without it, picking the
widgets of Moblin's Moblin Cloud settings; Moblin ignores an id that is not a widget of
that type, and pushes its `wheelOfLuck` again after the wheel one), which are Moblin's
own requests, with the scene, zoom preset, bitrate preset, macro and SRT connection ids
parsed as UUIDs like Moblin decodes them; anything else is a 422 `invalid-control`,
answered after the `watcher` check and the `control` access level (403 `no-control`,
except for `sendMessage`, which `view` may send); `setLive` with `on` true from an
assistant without `goLive` is a 403 `no-go-live` `The streamer has not let you go
live.`), sends it (`Streamers::control`, 5 s wait) and answers 202, with 409 `offline`,
504 `moblin-timeout` `Moblin did not answer. Try again.` and 502 `moblin-error` `Moblin
could not answer. Try again.`; a control that changes the settings (the SRT ones, the
scene ones and the Moblin Cloud ones) is followed by a `getSettings` while someone
watches. The same route takes the scene editor's requests from Moblin's
`remote-control-scenes-and-widgets` branch: `getScenes`, `getWidgets`,
`getVideoSources`, `getAlertsMediaGallery`, `getEditorOptions` (fonts, LUTs, scoreboard
players, the web remote, workout devices, stream deck layouts, Apple voices with each
voice's `language`, `showAllSettings`, `isPhone`, `forceSceneSwitchTransition`,
`workoutActive`, `inactiveVideoSourceSceneIds` and `canvasSize` (the stream canvas's
`width` and `height`, from the resolution and the portrait setting, which the outlines
need), the phone state Moblin's own settings views read), `getVideoSourceSnapshot`
(`id`, a JPEG of a video source widget's current frame for the mask and crop editors,
null for other widgets; the images of image widgets are never fetched, their outlines
are the layout box), `createScene`, `updateScene`, `deleteScene`, `createWidget`,
`updateWidget`, `deleteWidget` and `setScoreboardPlayers` (the protocol has no file
uploads, so no large files go through the backend or the remote control: the images,
models, gallery media and alert videos are picked on the phone, and the editor shows
what the phone has); the scenes, widgets, media items and players are Moblin's own
`SettingsScene`/`SettingsWidget` JSON, passed through as `serde_json::Value` and only
checked for a UUID `id` (`Control::valid`), the body is at most axum's default 2 MB, and
`getVideoSourceSnapshot` waits 60 s; a `get*` request answers 200 with Moblin's data
under its key (`Control::answers`: `{"scenes": [...]}`, `{"widgets": [...]}`,
`{"videoSources": [...]}`, `{"gallery": {...}}`, `{"options": {...}}`, `{"image":
"<base64>" | null}`), 502 `moblin-error` without any, and a Moblin `error` to any of
these (`Control::editor`) is 409 `refused` `Moblin refused the change. Reload the scenes
and try again.`, since Moblin answers it for an unknown or taken id; the log line names
the request, never its body. The dashboard (`Admin` in `pages/Dashboard.tsx`) has a
Streamer picker at the top for anyone who assists a channel (the picked channel kept per
tab in the URL as `/dashboard/<platform>/<login>`, matched against either of the
channel's logins, so each tab can show its own streamer and be bookmarked, with
`sessionStorage` as the fallback when the URL has none; "You" for their own page);
picking a channel shows that streamer's dashboard (`SignedIn` fetches the page's
account, then `Admin` with `page` set, whose calls in `data/account.ts` go to the
`/account/pages/{id}` routes) without the Payments, Assistants and Moblin sections, with
the same price settings as the streamer (the needs-payments card tells an assistant the
streamer has to connect PayPal or Stripe; the settings' `payable`, kept by
`without_payments`, is whether the page can take payments, which the dashboard uses in
place of the merchant and Stripe account ids). With the fake Twitch and the fake Kick,
any login resolves to a user (except `nobody`).
