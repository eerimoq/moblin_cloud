# backend devices

[Architecture index](../architecture.md). Keep this note current with its implementation.

`images.rs` — the banner image of a page. Only premium accounts (the `twitch:<Twitch
user id>` or `kick:<Kick user id>` in `--premium-accounts`, read like `--admins`, env
`PREMIUM_ACCOUNTS`, matched against any linked identity, plus admins; `GET /account`
answers `premium`) may upload one, with `POST /account/image` (the file as the raw body,
at most 2 MB), which the dashboard's Tip page section shows to them only. `banner`
decodes a JPEG, PNG, WebP or GIF of at least 600 pixels wide and at most 4000 pixels on
each side, crops it to 3:1 around the center and re-encodes it as a 1500×500 JPEG, so
nothing but pixels is kept; it is saved in the `page_images` table with a version (a
hash of it), answered as `image` with the page's settings and look, and served by `GET
/pages/{id}/image?v=<version>` with a year long immutable cache. Counted as
`imageUploads`. `POST /account/image/remove` works for everyone, and taking a streamer
off `--premium-accounts` only stops new uploads; the operator deletes the row to take
one down.

`app_attest.rs` — verifies Apple App Attest attestations and assertions so only the
Moblin app can connect to Moblin Cloud (a copy of the Moblin website's, which checks its
live post with it): `Attestation` is the base64 fields a client sends (`keyId`,
`attestation`, `attestationChallenge` and the `assertion`, all optional so a missing one
is a clear error; a malformed `appAttest` in Moblin's identify is read as an empty one,
so it fails as `invalid` instead of making the whole message fail to parse), and
`verify` checks them against some client data, the hello challenge of the WebSocket.
Stateless by design: the app resends its attestation every time and the server
re-verifies the chain each time, deliberately ignoring certificate validity dates. Tests
use public sample vectors in `src/testdata/`.

`remote_control.rs` — the assistant side of Moblin's remote control protocol, for tips
and wheel spins on the page. Moblin Cloud connects with the key in the URL and an empty
password, which the backend does not check. Moblin proves it is the Moblin app with App
Attest in `identify`: an optional `appAttest` with the same `keyId`, `attestation` and
`attestationChallenge` its live post to the Moblin website sends (the key attested once
per device for the live post is reused) plus an `assertion` whose client data is the raw
bytes of the hello `challenge`, the fresh random 64 bytes the backend sent on this very
connection, so no separate challenge round trip or `Challenges` entry is needed
(`Identity` from `identify`, checked by `AppAttest::verify` in
`handle_remote_control_streamer`). Unless `--allow-unattested`, every connection is
verified (logged when it fails: no `appAttest`, an older Moblin, or an invalid one), and
with `--require-attested-moblin` (env `REQUIRE_ATTESTED_MOBLIN`) the failed ones are
refused like one without a device id. Moblin Cloud is off in Moblin on Mac, where App
Attest is not supported. Serde types mirror Swift's synthesized `Codable`
(`{"ping":{}}`, optional fields omitted), and must match
`Moblin/RemoteControl/RemoteControl.swift` in the Moblin repo. Moblin pushes its wheel
(the wheel of luck widget picked in Moblin's Moblin Cloud settings) as a `wheelOfLuck`
event on connect and on every change; it is kept with the session and served by `GET
/pages/{id}/spin` as `wheel` for the page to draw, next to `connected`, so the page can
tell a streamer who is offline from one who picked no wheel (whose spin orders are
refused with 409). Moblin also pushes the icon shown in the top right of its UI (the one
picked in its Store settings, `iconImage`) as an `icon` event
(`{"icon":{"id":"AppIconKing"}}`) on connect and on every change; `serve` hands a known
one to its caller, which saves it as the page's (`Pages::set_icon`, the `page_icons`
table, the last one any device sent), and unknown ids are ignored. The requests sent are
`tip`, shaped like Moblin's `music` request: viewer `name`, `message`, `requestId` and a
`kind` (`TipKind`, Moblin's `RemoteControlTipKind`) of `message` (amount as a string
like `7.00` and currency), `effect` (for a sound effect its Moblin name as `sound`, like
`Bad chili fart`) or `wheelOfLuck` (number of spins as `count`), answered `ok` without
data and deduplicated by Moblin on `requestId`; and the monitoring and control of a
device for the device page (`Streamers::watch`, `control`). Each watched device (page
and device, whether or not it has a session) has a `Monitor` in `Sessions::monitors`,
created by the first watcher's `watch` and removed with the last: the number of
`watchers` (open event streams), the merged `status` (Moblin's
`RemoteControlStatusGeneral`, `TopLeft` and `TopRight` as `general`, `topLeft` and
`topRight`, each replaced whole when a `status` event carries it), the merged `state`
(Moblin's `RemoteControlAssistantStreamerState`, the keys of each `state` event written
over the old), the last `settings` (Moblin's `RemoteControlSettings`, from the `data` of
any `getSettings` response, whoever asked; its `moblinCloud`, missing from older
Moblins, holds the `alertsWidgetId` and `wheelOfLuckWidgetId` of Moblin's Moblin Cloud
settings, each omitted while none is picked; the widgets to pick from come from
`getWidgets`) and a broadcast channel of `Update`s that every watcher's stream reads; it
outlives the device's sessions, so watchers keep it when the device reconnects. The
first watcher's `watch` sends `startStatus` (interval 1, filter `topRight` true) and a
`getStatus`, every watcher's a `getSettings`, all without waiting for the answer
(`Session::send`; the settings and status answers are picked up in `handle`, a
`getStatus` answer merged like a `status` event but without a sample), and a task per
watched device (`poll_status`, spawned with the monitor) sends `getStatus` every 5 s
until the last watcher is gone, since the periodic event only carries the top right
part, a device connecting while it has watchers is sent them again (`resume_watching`)
and the last watcher's guard sends `stopStatus` when dropped, so Moblin pushes status
only while a page is open, and a device that lost its connection or connected tells the
watchers `connected` false or true. Controls (`Control`, serialized flat as Moblin's
requests through the untagged `Request::Control`) wait 5 s for the `ok` like tips.
Moblin's Moblin Cloud side is `MoblinCloud` in Moblin's `ModelMoblinCloud.swift`: on
`startStatus` it sends the full state and then, from Moblin's 1 s timer, a `status`
event built with the request's filter like its regular remote control streamer does (so
only `topRight`, the general and top left parts coming from the backend's `getStatus`
every 5 s), until `stopStatus` or the connection drops; it forwards `state` changes
while asked, answers `getSettings` with Moblin's settings and forwards the controls
above to the `Model` methods shared with its remote control streamer's delegate
(`remoteControlSetLive`, `remoteControlGetSettings` and so on in
`ModelRemoteControl.swift`, or the plain ones like `selectScene`; neither delegate calls
the other's `remoteControlStreamer*` methods). `topRight` carries, besides Moblin's
display strings, `bitrateInfo` (`bitsPerSecond` and `totalBytes`, while live) and
`bondingInfo` (per SRTLA or RIST connection its `name`, `bytesPerSecond` and `rtt` in
milliseconds, while bonding), added to Moblin's `RemoteControlStatusTopRight` for the
graphs. The preview: `GET /account/pages/{id}/devices/{device}/preview`
(`api/devices.rs`, the same checks as the event stream but with `previewer` in place of
`watcher`, so an assistant without `preview` gets 403 `no-preview` `The streamer has not
let you see the stream preview.`, also on the minute's recheck, which ends an open
stream once it is turned off) is a server-sent event stream of `frame` events (`image`,
the base64 image of Moblin's `preview` message, at most 2 MB); its first watcher's
`Streamers::watch_preview` sends `startPreview`, a device connecting while it has
watchers is sent it again, the last watcher's guard sends `stopPreview`, and the last
frame is kept for new watchers (`PreviewMonitor` in `Sessions::previews`, apart from the
status monitors). Moblin Cloud forwards these controls to the `Model` methods its remote
control streamer uses too (`remoteControlSetFilter`, `remoteControlSetPreviewStream`,
`remoteControlSendMessage`, `setGimbalTracking`, `startMacro`, `stopMacro`), and between
`startPreview` and `stopPreview` sends a `preview` frame of Moblin's low fps image, at
least one a second (`MoblinCloud.requestingPreview`, counted in `setLowFpsImage`), only
in Moblins from after these were added; older ones ignore them. Older Moblins answer
empty settings, push nothing and ignore the controls.

`effects.rs` — effects, the third paid kind next to tips and wheel spins
(`Item::Effect`, `kind` `effect`, counted as `effectOrders`, `effectPayments`,
`effectTests`, `effectDeliveries` and so on): the viewer pays the page's one effect
price (the page setting `effects`, `pages::Effects`) to have Moblin play the effect they
picked, with an optional name and message. Only sounds for now (`kind` `sound`), the
fart sounds of Moblin's `Alerts.bundle`, copied into `src/effects/` and compiled in,
each with an id (`fart`, `fart-2`, `bad-chili-fart`, `perfect-fart`), its Moblin name
and its content type. `GET /pages/{id}/effects` answers the paid `effects` (the price
and the payment fields like the tips answer, null when off), `subEffects` and the
`options` (each effect's `id` and `kind`), `GET /effects/{id}` serves the media so the
page plays a sound when the viewer taps it, `POST /pages/{id}/effects/orders` takes
`effect` (400 `no-such-effect` for an unknown one) and the shown `price` (400
`invalid-option` when it changed), and the delivery goes to every device like a tip, as
a `tip` request of kind `effect`. Subscribers get effects without a payment like wheel
spins (`subscribers.rs`, the page setting `subEffects`). `GET
/account/recent?kind=effect` answers the `effect` of each item, and the dashboard's test
effect sends the picked one (`effect` in `POST /account/test/effect`, the first one
without it).

`icons.rs` — the icons Moblin shows in the top right of its UI (its
`AppIcon*NoBackground` images, the plain one and the Store's, copied into `src/icons/`
downscaled to 256 pixels high and compiled in), each with an id (`plain`, `king`,
`san-diego` and so on) and its Moblin id (`AppIcon`, `AppIconKing`, ...). `GET
/icons/{id}` serves the PNG; the page's `icon` is shown on the tip page below the
channel link, or right of the streamer's avatar, cut out of the banner like the avatar,
when the page has a banner (`MoblinIcon` in `pages/Page.tsx`). A new Store icon in
Moblin needs a row here, or the backend ignores it.
