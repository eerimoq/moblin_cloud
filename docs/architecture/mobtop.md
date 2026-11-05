# Terminal client

[Architecture index](../architecture.md). Keep this note current with its implementation.

`mobtop [--backend-url <url>]` (env `MOBTOP_BACKEND_URL`, `https://cloud-api.moblin.app`
by default): the web Monitor (`pages/Monitor.tsx`) in a terminal, like `top` for a
stream, with everything it does, for the signed in account's own page and the channels
it assists, picked in a channel menu (`menu::channels`: You and each assisted channel
with its platform and login, the shown one ticked) opened with `c` or a click on the
header. `main.rs` runs one tokio `mpsc` of `app::Message`s fed by crossterm's
`EventStream` (keys and, with mouse capture on, the mouse), a 1 s `Tick` (which also
saves the layout) and the backend calls and streams, and redraws after each;
`App::update` in `app.rs` holds all state (`Screen`: `SignedOut`, `SigningIn`,
`Connecting`, `Loading`, `SignedIn(Monitor)`) and spawns the backend calls, whose
results come back as messages tagged with the page (and device) they are for, so ones
for a channel no longer shown are dropped. The layout is the web's: `layout.rs` ports
`components/Tiles.tsx` (`Pane` of `Split`s and `Leaf`s, serialized exactly like the
web's so both read and write the same saved layout, each leaf with an in-memory `id` the
focus, maximize and selections follow; `sanitize`, `default_layout`, `split_at`,
`remove_at`, `move_to`, `move_to_edge`, `same_arrangement`, `resize` with the 0.08
minimum, `added`, and `areas`, which turns the size fractions into terminal rects and
dividers), loaded with `GET /account/pages/{id}/monitor` when a page opens (unless a
save is pending) and saved with `POST` a second after the last change, when switching
channels, signing out and before quitting. Each device leaf (phone, graphs, status,
message) shows its `device` (kept by the leaf when its panel changes, like the web's)
or, like the web's `deviceOf`, the first connected one else the first; `reconcile`
(`wanted_streams`) keeps one device event stream and `Feed` (`feed.rs`, a port of
`components/deviceFeed.ts`) per device some leaf shows and one preview stream per device
a phone leaf shows without `hidePreview` while the account answer's `preview` is true,
like the reference counted `useDeviceFeed` and `usePreviewFeed`; the page's change
stream refetches the shown page's account and the activity (`sse.rs`: backoff up to 10
s, a 401 signs out, another 4xx ends it). The panels (`panes/`): Phone (`phone.rs`: a
status line with live, recording, muted, battery, thermal state and the audio level bar,
the web's notices about the preview, control and go live permissions, the preview,
through the terminal's own image protocol when it has one (kitty, sixel or iTerm2,
detected once at start by `ratatui-image`'s `Picker::from_query_stdio` before the key
reader starts, since it reads the answers from stdin; each device's latest frame is kept
in `Monitor::images` (`Picture`), the phone pane only draws the last encoded one
(`ResizeEncodeRender::render`, which never encodes) and asks for its size in `wanted`,
and after each draw `App::encode_previews` scales and encodes, in a blocking task, any
picture whose frame or wanted size changed (`Message::Encoded`), so maximizing a pane or
a new frame never holds up the screen (the dependencies are built optimized even in
debug builds, `[profile.dev.package."*"]`, since `image`'s scaling and the zlib
compression are many times slower without); the encodings alternate between two kitty
image ids per device: a new frame under the same id would only rewrite the first
placeholder cell, since ratatui redraws only changed cells and kitty keeps the others on
the old image, while a new id changes every cell's color, and reusing the other id
bounds kitty to two images), else in true color `▀` half blocks (`preview.rs`), or the
state as text, and `enter` for the controls menu), Activity (`activity.rs`, the three
`recent` lists merged like `Activity.tsx`, the leaf's `kinds` toggled with `1`-`3` and
saved in the layout, `r` sends again, `m` loads more), Graphs (`graphs.rs`, bitrate and
round trip time `Chart`s), Status (`status.rs`, `status_rows` in the web's order, ending
with the socket's traffic from `Feed::traffic`), Message to the streamer (`message.rs`:
its prompt is always shown and, while the pane has the focus, every key but `tab` and
`alt`+arrows types into it (the leaf's draft in `Monitor::drafts`), `enter` sends it as
`sendMessage`, `esc` clears it), Toybox (`toybox.rs`, an ASCII whoopee cushion over the
list of fart effects, `account::EFFECTS`, picked with `↑↓` or a click; `enter` sends the
picked one as a test effect, `1`-`4` and a click send that one, and it shows Sent) and
Web page (`web.rs`: a terminal can't show pages, so it keeps the `https://` address or
the streamer's Twitch chat and `o` opens it in the browser). The controls menu
(`menu.rs`, items built from the device's `state` and `settings`, bodies from
`controls.rs`) is the replica's quick buttons in their order: Go Live/End and Record
behind the web's confirmations, Scene, Zoom preset, Torch, Mute, Mic, Stealth mode,
Bitrate, Connection priorities (the switch, each connection's enabled and its priority 1
to 10 with `←→`), Reload browser widgets, the 16 Filters (on from the alternating
`state.filters`), Instant replay, Auto scene switcher, Macros, Moblin Cloud (the Message
and Wheel of luck pickers of the phone's settings: the picked ids from the settings'
`moblinCloud`, the choices from a `getWidgets` fetched when the menu opens,
`Backend::query` and `Monitor::widgets`), Gimbal tracking and Preview stream; disabled
unless the device is connected and the access is `control` (Go Live also without
`goLive`), each pending control (`kind` or `kind:id`) refused until it answers, errors
in the status line. The pane menu (`space`) is the web's title bar and menu: Panel,
Device, Hide/Show stream preview, Maximize/Restore, Split right/down, Change web page,
Move pane, Remove pane, Reset layout and Copy layout to the caller's other pages;
besides it `tab`/`shift-tab` and `alt`+arrows move the focus, `z` maximizes, `<` `>`
resize the focused pane by 0.02, `M` moves it with the keys (arrows pick the target,
`enter` swaps, `hjkl` beside it, `HJKL` to a window edge), `s` signs out and `q` quits;
with the mouse a click focuses a pane or picks a menu entry or activity item, a click on
a pane's title row without dragging opens its pane menu (the only way to it from the
message pane, which takes `space`), the wheel scrolls, a divider (the upper pane's
bottom border for a horizontal one, so title rows stay free) drags to resize and a
pane's title row drags onto another pane's zones (`zone_at`, like the web's `zoneAt`) or
the window's outer rows and columns to move it, highlighted while dragging and refused
when `same_arrangement` says nothing changes. Below 80 columns only the focused pane is
shown. Signing in (`sign_in.rs`) is the native app flow of RFC 8252 with PKCE: it makes
a random verifier, listens on `127.0.0.1:0`, offers the platforms the `signIn` of `GET
/config` answers (none until it does, both when it fails), opens
`/auth/<platform>/login?port=<port>&challenge=<base64url SHA-256 of the verifier>` in
the browser (and shows the address in case none opened), takes the one-time code from
the backend's redirect to `http://127.0.0.1:<port>/?code=<code>` (or `?signInFailed`),
trades it with the verifier for the session at `POST /auth/session` and only then
answers the browser a small page saying whether it worked; `c` signs in on another
device instead (`Screen::Connecting`, for SSH and headless machines): `POST
/auth/connect` gives a device code and a user code, mobtop shows the user code, the
Connect page's address and a QR code of the address, without the code so it has to be
typed (`qr_lines` in `ui.rs`, `qrcode`'s half blocks drawn black on white, left out when
the terminal is too small), and polls `POST /auth/connect/session` every `interval`
seconds until it answers the session (a 4xx, denied or expired, ends it with the
backend's sentence); `session.rs` keeps the session per backend URL in `sessions.json`
in the platform's config directory (`dirs::config_dir()/mobtop`), mode 0600.
