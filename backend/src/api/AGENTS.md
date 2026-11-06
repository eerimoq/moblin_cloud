# API

The router, `Api`, `Failure` and the session checks are in `../api.rs`; the handlers are here by feature, each with its tests.

## Access

- A public-viewer disable switch must not interrupt existing payment captures, provider webhooks, device connections or authenticated dashboard streams. Classify new routes explicitly and extend the gate tests when adding a viewer endpoint.

- `page_user` loads the page's account once and merges the assistant's `Rights` over both platforms, the owner getting `Rights::all`. Handlers take the page's account and the rights from the `PageUser` instead of looking them up again.
- Pick the check that matches the route: `assistant()`, `watcher()`, `previewer()` or `editor()`.
- Assistants are an allow list kept by hand; Moblin Cloud never asks Twitch or Kick who moderates a channel.
- The assistant tables hold platform user ids on both sides, never account ids, so a rename changes nothing and a channel's list survives unlinking and relinking it.
- `editTips` covers the settings and banner routes; test sends and Send again are always allowed.
- `goLive` only limits going live: ending the stream is always allowed.
- `sendMessage` only needs `view`, and its `name` is always set to the caller's account name, whatever the page sends.
- An assistant's answers go through `without_payments`, and items in their recent list have no `provider`. `payable` is kept, so the dashboard can tell whether the page takes payments.
- An assistant saving settings keeps the page's currency, whatever the request says.

## Streams

- The device, preview and `/account/pages/{id}/events` streams check the caller again every minute and close when it fails, so signing out everywhere, being removed or losing access ends an open stream. Those rechecks also keep the assistant active.
- A known device that is not connected gives a stream too, so an open device page never gets errors while waiting for its Moblin.
- `changes.rs` keeps a broadcast channel per page only while someone listens. `Change::Page` is for what the public page shows, `Change::Account` for what only the dashboard shows and `Change::Recent` for orders and deliveries.

## Controls

- `remote_control::Control` is Moblin's own requests; ids are parsed as UUIDs like Moblin decodes them.
- A control that changes the settings is followed by a `getSettings` while someone watches.
- The scene editor's scenes, widgets, media items and players are passed through as `serde_json::Value`, only checked for a UUID `id` in `Control::valid`, so the backend does not follow Moblin's settings types. A new `get*` request needs its key in `Control::answers`.
- A Moblin `error` to a scene editor request means an unknown or taken id, so it is answered 409 `refused`.
- The log line names the request, never its body.
- No large files go through the backend or the remote control.

## Orders and sends

- Viewer order routes take `provider`, PayPal by default.
- Test sends skip payment but run the same Moblin checks as paid ones, and are limited per page in memory (`Api::tests`). Test deliveries are not listed in recent.
- A captured order is kept 30 days, so that is the whole history of `recent`.
- `GET /account/recent` without `kind` answers every kind in one request, which the dashboard, the Activity panel and mobtop use on every `recent` event; `before` is only for paging one kind.
- The body of `POST /account/pages/{id}/monitor` is stored as it is, so the backend does not follow the layout format.

## Admin

`GET /admin/stats` is for the site operator, from `--admins` (a bare id is a Twitch one, and any other prefix is refused at start). Unfinished orders and pending deliveries are plain column counts, and no JSON is read.
