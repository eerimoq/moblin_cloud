# frontend

[Architecture index](../architecture.md). Keep this note current with its implementation.

**Multi-page, no router.** Each page is its own HTML entry listed in `vite.config.ts`
(`dashboard/index.html`, `page/index.html`, ...) with its own `src/*.tsx` entry that
renders a page component. Sub pages live in a directory so GitHub Pages serves clean
URLs like `/account/`. The dashboard is `index.html`, at the site root
(`pages.dashboard`); `dashboard/index.html` renders it too, for the backend's sign in,
PayPal and Stripe redirects and old links, and `src/dashboard.tsx` replaces
`/dashboard/` with `/` in the address bar, while an assisted streamer's dashboard stays
under `/dashboard/<platform>/<login>` (`pages.assisted`), since `/<platform>/<login>` is
their tip page. Add a page by adding all three: the HTML file, the entry, and the
`rollupOptions.input` line. `404.html` is what GitHub Pages serves for unknown paths; it
renders the page for `/twitch/<login>` and `/kick/<login>` (a signed in streamer's page
under the login of that linked platform, showing that platform's name, avatar and
channel link; both addresses show the same page and settings), the dashboard for
`/dashboard/twitch/<login>` (an assisted streamer's dashboard) `/device/<page
id>/<device id>` (a device's remote control, in the dashboard) and `/monitor/<page id>`
(the monitor, in the dashboard) and a not found page otherwise. Streamers sign in and
manage their page at `/`; `/page/?streamer=<id>` shows a page by id and redirects to the
dashboard without one. The page header shows a link to the dashboard when the browser
holds a streamer session (`readSession`) whose account owns the page (its `settings.id`)
or assists it (`assisting`), to `/dashboard/<platform>/<login>` for an assistant under
the platform the page is shown on (`dashboardLink` in `pages/Page.tsx`). Signing in and
linking are `PlatformSignIn` in `components/PageParts.tsx` (`signInUrl` and `linkUrl` in
`data/account.ts`, the latter `/auth/<platform>/login?link=1`), offered only for the
platforms the backend has credentials for (`signInPlatforms` in `data/account.ts`, from
the `signIn` of `GET /config`, none until it answers, so an unconfigured one never
flashes, and both when it fails; an account's linked platforms stay listed under Linked
accounts, and the assistant lists of an unconfigured platform lose their add form);
Account (`AccountView` in `src/pages/Account.tsx`, loaded lazily, at `/account/`, whose
`account/index.html` renders the dashboard like `admin/index.html`, opened in place from
Account in the dashboard's top right menu and, like Admin, not per page (`accountPage`
in `ownPages`), shown in the page picker while open but not listed there) holds the
Linked accounts section (`LinkedAccounts`), which links and unlinks platforms, Sign out
everywhere and Delete account; the dashboard replaces any address with `/` while signed
out; `createAccountSession` posts the `#link=<token>` the backend's link callback
returns to `/account/` to `POST /account/link` with the stored session (so a link only
ever lands on the account signed in on that browser), showing the result there
(`linkResult`, passed down from `Dashboard`); the payment providers are offered the same
way: `paymentProviders` in `data/account.ts`, from the `payments` of `GET /config`,
whether PayPal and Stripe are configured, both until it answers and when it fails, for
the texts that name them (`paymentProvidersText`, the landing page's steps, Terms and
the assistants' needs-payments card, since an assistant's `settings` say neither is
configured; the signed in streamer's Payments section goes by `settings.paypal` and
`stripe`; the admin page goes by its stats' `integrations`, showing only the configured
ones' rows, tiles and Providers section, while its Server section still lists both as On
or Off); the Assistants section is hidden while the account has neither Twitch nor Kick
linked, and the subscriber spins settings while it has no Twitch linked
(`account.twitch`). The dashboard and the remote control of each device are one page
(`SignedIn` in `pages/Dashboard.tsx`), sharing the header: the page picker (`PagePicker`
in `components/HeaderPickers.tsx`: Dashboard first, then Monitor (`monitorPage`, a
sentinel no `streamerId` can be), then Admin for an admin (`adminPage`, likewise), then
the page's devices from its account answer, which `SignedIn` fetches for the dashboard
and the devices alike, keeps per page and refreshes on the page's change stream, listed
whatever the caller's access, the device then saying the streamer has not let them use
the remote control under No access), then, for an assistant, the streamer picker
(`StreamerPicker`), then the account menu. Picking switches the page in place with
`history.pushState` between `/` (or `/dashboard/<platform>/<login>`), `/admin/`,
`/account/` and `/device/<page id>/<device id>` (rendered by `404.html`, with the dev
server's rewrite in `vite.config.ts`), keeping the shown device until the new one's
snapshot arrives; the Remote control link of each device in the dashboard's Moblin
section, which assistants see as a read-only device list, switches in place too, and
picking a streamer while a device is shown opens that streamer's first device, connected
ones first, or their dashboard when they have none, and while the monitor is shown that
streamer's monitor. The monitor (`Monitor` in `src/pages/Monitor.tsx`, loaded lazily)
shows everything needed to watch and control a stream on one page filling the window:
`SignedIn` then makes the header full width (`CloudHeader`'s `wide`; it has the same
compact height on every page), drops the `Band` and hides the footer (`fullWindow`, read
by `DashboardPage`). The tiled layout (`wide` in `src/viewport.ts`, the `md` breakpoint
Tiles stacks the leaves below, where the page scrolls like the dashboard) is `h-dvh`, so
it grows when the browser's bars go. On a small screen, either side under 500 px
(`small`: a phone in either orientation), the header hides on scrolling down and shows
on scrolling up, like the browser's own bars (`scrolledDown`, from the window's scroll,
10 px past the last turn, always shown at the top): taken out of the flow in the tiled
layout, so the panes take its room, and slid up (`CloudHeader`'s `hidden`) in the
stacked one, where the sticky header taking its room would move the page; the tiled
layout is then `sticky top-0` over a `50dvh` spacer, so a swipe on a pane scrolls the
page under it like any page, which is what a phone browser collapses its bars on (one
bar's height of scroll room, as `h-lvh` gives, is not enough for Safari in landscape),
and the panes grow into the room; nothing collapses them without a swipe. The header's
Full screen button (and Full screen/Leave full screen at the top of each pane's menu,
the way back while the header is gone; both only where they do something,
`fullscreenOffered`: not on a small screen without the Fullscreen API) turns on the
monitor's full screen mode (`fullscreen` in `src/fullscreen.ts`), which hides the header
and, where the browser has the Fullscreen API (desktops, Android, iPad, not iPhone,
whose Safari only has it for videos), puts the document in the browser's full screen
mode, hiding the browser's bars; Esc or the browser's own gesture leaves that and the
mode with it (`fullscreenchange`, only when the mode entered it, so a video going full
screen in a pane does not end it), and leaving the monitor leaves the mode. Its layout
is a tree of panes (`components/Tiles.tsx`: `Split`s, rows or columns with `sizes` as
exact fractions of the split (the gaps are each pane's padding and the dividers lie over
the boundaries, so splitting or removing a pane never moves its neighbours) and dividers
dragged with the pointer or the arrow keys, and `Leaf`s, each a panel; below the `md`
breakpoint the leaves are stacked and the page scrolls), each pane with a slim title bar
picking its panel and, for device panels when the page has several devices, its device
(Automatic: the first connected, else the first; the leaf keeps its `device` when
another panel is picked, one without a device picker too, so picking a device panel
again shows the same device), and a menu to split it right or down, remove it, reset the
layout and copy it to the caller's other pages (their own and those they assist, saved
there with the same `POST`); a button in the title bar maximizes a pane over the whole
pane area until it is restored or the layout changes other than through the pane itself
(`maximized` in `Tiles`, not saved; the other panes are only hidden, so nothing in them
reloads); a pane is moved by dragging its title bar onto another (`moveTo`, HTML drag
and drop with an overlay per pane, `LeafSlot`, that also covers iframes while dragging):
dropped near an edge it goes on that side of the target, in the middle the two swap, and
dropped on one of the strips along the window's edges (`WindowEdges`, `moveToEdge`) it
spans the whole width or height there, taking 30% of the window, added to the root split
when that runs the same way; a target that would leave the panes arranged as they are
(`sameArrangement`: the order of the panes in their rows and columns, sizes aside and a
split nested in one running the same way counted as part of it) is neither highlighted
nor accepted. Panels: Phone (`MoblinScreen` with `fit`, scaled to the pane's width and
height, whose preview the Hide stream preview button in its title bar, like the device
page's, hides and shows again, `hidePreview` in the leaf, closing its preview stream to
save the device's traffic, and the Show as a window button draws it without the phone
(`window` in the leaf, `MoblinScreen`'s `window`: no bezel, notch or rounded screen, a
control bar without the room for the screen corner, the screen as large as the pane at
Moblin's point sizes, at most one CSS pixel per point and at least 640×393 points,
scaled down below that)), Activity (`components/Activity.tsx`: the tips, spins and
effects of `createRecent` in one feed by time, only items newer than the oldest loaded
item of each kind with `more`, compact rows with an icon Send again button, kind filter
chips kept in the pane, new arrivals highlighted with `arrival-flash`), Graphs
(`components/Graphs.tsx`, `TimeChart` with `fill`), Status (the `general`, top left and
top right items as a dense list), Message to the streamer (`components/MomMessage.tsx`,
`compact`), Toybox (`ToyboxPanel`: buttons that play things on the phone, for now a
whoopee cushion per fart effect (`FartButton`, one per `effects` in `data/effects.ts`,
each with its own `WhoopeeCushion`, inline SVG in the `--color-cushion-*` tokens, the
same in both themes, squished with `cushion-squish` when pressed) sending that effect as
a test effect, `sendTest`, so it goes to every device of the page like the dashboard's)
and Web page (an iframe of an `https:` address, or the Twitch chat embed of the page's
Twitch login with `parent` set to the current host, following the streamer picker). The
device panels share one event stream per device, and the previews one per device,
through the reference counted `useDeviceFeed` and `usePreviewFeed` in
`components/deviceFeed.ts`, which `DeviceView` uses too; a feed is kept 3 s after its
last user goes, so switching a pane's panel or page reuses its stream instead of sending
Moblin `stopStatus` and `startStatus` again. The layout is saved per signed in account
and page with `POST /account/pages/{id}/monitor` a second after the last change (and on
`pagehide`), so an assistant keeps one layout per streamer they assist; `sanitize`
checks what `GET` answers, falling back to the default layout. A device (`DeviceView` in
`src/pages/Device.tsx`, loaded lazily) monitors and controls one Moblin device for the
streamer and their assistants, the content right under the header like the dashboard. It
subscribes to `GET /account/pages/{id}/devices/{device}/events` (always under the page
id, the owner's being `settings.id`), a server-sent event stream read with `fetch` and
the bearer session in `src/sse.ts` (`subscribe`: parses `event:`/`data:` blocks,
reconnects with backoff, closes 30 s after the tab is hidden and reopens when shown, so
a hidden page costs nothing for long and a quick tab switch keeps the stream), whose
`snapshot` gives the device's `name`, `connected`, `status`, `state`, `settings` and
`traffic`, followed by `status`, `state`, `settings` and `connection` events the page
merges in, keeping the last 60 samples of the `status` events for the graphs in the page
only (a reload starts them over). Two sections picked in a left menu like the
dashboard's (`SectionMenu` and `createSectionChoice` in `components/SectionMenu.tsx`,
shared with the dashboard: the chosen section in `location.hash`, a dropdown on narrow
screens), each a card: Phone (`MoblinScreen` in `components/MoblinScreen.tsx`, with the
panels and the settings tree in `MoblinPanels.tsx` and the iOS-like rows and switches in
`MoblinParts.tsx`; Moblin's settings types and helpers in `data/moblin.ts`): a replica
of Moblin's landscape UI on an iPhone, drawn at Moblin's point sizes on an 852×393
screen and scaled to the card, built only from what Moblin's remote control protocol
gives, the device's `state`, `status` and `settings` (no chat, which the backend does
not relay, and none of Moblin's other settings, so colors, big buttons and the like are
Moblin's defaults): the stream preview (the frames of `GET .../preview`, which the page
only opens while the Phone section is shown, the account answer's `preview` is true and
the viewer has not hidden it with the Hide stream preview button next to the section's
title, whose tooltip says it saves bandwidth on the phone (`previewHidden` in
`localStorage`, to save the device's traffic), refetched on the page's `account` change
event; otherwise the card says the streamer has not let you see it), the top left and
top right statuses, tapped to switch between icons and text (`statusCollapsed` in
`localStorage`), the mute indicator, the zoom preset and scene pickers in the bottom
right, and the control bar with the battery, thermal state, the page's Moblin icon, the
settings button, the quick buttons laid out like Moblin's (from the bottom right, pages
picked with the dots below Go Live/End) and Go Live/End; a quick button either sends its
control (Torch, Mute, Record, Stealth mode, Gimbal tracking, Preview stream, Instant
replay, Reload browser widgets, the filters as `setFilter`), opens a panel next to the
control bar (Mic, Bitrate, Connection priorities, Auto scene switcher, Macros), which,
like the settings, has Moblin's hide and close buttons in its top right (`PanelButtons`:
hiding shrinks the panel to 1 point, keeping what it shows, and moves the buttons over
the stream with a background; opening or closing any panel shows it again, like Moblin's
`panelHidden`); the settings button opens the settings the protocol's `settings` and
`state` hold (macros, and Moblin Cloud: the Message and Wheel of luck pickers of
Moblin's Moblin Cloud settings, the alerts widget that shows viewers' messages and the
wheel of luck widget they spin, the picked ids from the settings' `moblinCloud` and the
choices from `getWidgets` fetched when the Moblin Cloud page opens, filtered by type,
set with `setMoblinCloudAlertsWidget` and `setMoblinCloudWheelOfLuckWidget`, None
without an `id`; a Moblin whose settings have no `moblinCloud` is told to update), each
changed with the controls above; below the replica, a message to the streamer
(`MomMessage`) sends `sendMessage`, which Moblin shows in its chat on the phone as from
the sender (the backend fills in `name` with the signed in account's name, whatever the
page sends; Moblin shows Mom without one), disabled like the controls; the quick buttons
are the replica's own fixed list (`quickButtons`) of the ones it can operate, since the
protocol does not say which buttons the phone shows, on three pages: the basics, the
filters and the rest, each on or off from the `state`; the phone's colors are the
`--color-phone-*` tokens, the same in both themes; the Show as a window button next to
the section's title draws it like the Monitor's window instead, filling the card to the
window's height, `phoneWindow` in `localStorage`) and Last minute (`Graphs`: `TimeChart`
line charts drawn as inline SVG from the samples: the bitrate, with the total as a
thicker ink line and each bonding connection's bits per second in its own hue, and the
round trip time per connection, each with a crosshair tooltip and a legend with the
latest values; the connection colors are the `--color-chart-*` tokens in `index.css`,
validated for both themes). Connect (`ConnectView` in `src/pages/Connect.tsx`, loaded
lazily, at `/connect/`, whose `connect/index.html` renders the dashboard like
`account/index.html`; `connectPage` in `ownPages`, shown in the page picker while open
like Account) approves or denies a mobtop sign in on another device with `POST
/account/connect`: the code is always typed, never taken from the address;
`src/dashboard.tsx` marks a visit to `/connect/` in `sessionStorage`
(`connectPendingKey`) since signing in loses the address, `readTarget(true)` opens
Connect while it is there (`pendingConnect`, which also makes `CloudLanding` ask to sign
in), and `ConnectView` clears it. Admin (`AdminView` in `src/pages/Admin.tsx`, loaded
lazily) shows the operator's statistics from `GET /admin/stats/events` (the `stats` that
`GET /admin/stats` answers, pushed every 2 s) in the dashboard shell at `/admin/`
(`admin/index.html` renders the dashboard, which reads the path), picked in the page
picker only by an admin (`admin` in the account answer; anyone else is sent to the
dashboard) and kept whatever streamer is picked, since it is not per page (picking a
streamer while it is shown opens their dashboard); it stays English. It opens with a
status banner and a Problems table that sum the failure reasons which are our, PayPal's,
Stripe's or Moblin's fault (`problems` and `problemReasons`) across tips and spins, with
PayPal and Stripe failures as separate rows from the per provider counters, which the
Providers section shows in full (orders and payments per provider); viewer mistakes and
offline streamers are listed per kind below in muted rows, never counted as problems.

**Base path is dynamic.** `vite.config.ts` uses `/` when `public/CNAME` exists and
`/moblin_cloud/` otherwise. Anything referencing a file under `public/` must go through
`asset()` in `src/asset.ts`; never hardcode a leading slash in a src attribute.

**Content lives in `src/data/`, not in components.** URLs in `links.ts` (the Moblin
website's as `links.website` and `links.docs`), effects, Moblin's settings and the
remote control types in their own files. Components in `src/components/` are the parts
the pages are composed of.

**Backend access** goes through `data/backend.ts`: `links.api`: `backendUrl` in
`config.js`, a plain script every HTML entry loads before its module, so the backend is
configured at deploy time, not build time: `public/config.js` has `/api` (proxied to the
backend by the dev server, and by Caddy in the frontend image), and the Deploy frontend
workflow overwrites it in `dist/` with the repository variable `BACKEND_URL`
(`https://cloud-api.moblin.app` for cloud.moblin.app), which GitHub Pages needs since it
can't proxy `/api`. A new HTML entry needs the `<script src="/config.js"></script>` too.
Nothing is polled: everything that changes on the backend is pushed as server-sent
events read with `subscribe` in `src/sse.ts` (closed 30 s after the tab is hidden, so a
hidden page costs nothing for long, reconnected with a jittered backoff, and not retried
after a 4xx until the tab is shown again). The dashboard (for its sections and the page
picker alike) and the tip page use change streams that only name what changed
(`subscribeChanges`: each event name maps to a refetch through the normal GET routes,
run 300 ms after the first event so a burst, like the `recent` events of a tip reaching
several devices, is one refetch, and every refetch runs again when the stream reopens,
since events may have been missed while it was closed): `GET /account/pages/{id}/events`
for the owner and assistants (`account` refetches the account, `recent` the received
lists) and the public `GET /pages/{id}/events` (`page` refetches the tips, spin and
effects answers and the viewer's allowances).
