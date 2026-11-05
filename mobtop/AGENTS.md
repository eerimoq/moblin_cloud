# mobtop

The web Monitor (`pages/Monitor.tsx`) in a terminal, with everything it does, for the signed in account's own page and the channels it assists.

`just dev-mobtop` runs it against `http://localhost:5173/api`, through the dev server like the frontend, so `just dev` (or `dev-backend` and `dev-frontend`) has to run: the sign in callbacks go to `dev-backend`'s `--backend-url`, which is that address, and browsers bind the state cookie to the port the sign in started on.

- `main.rs` runs one tokio `mpsc` of `app::Message`s and redraws after each; `App::update` in `app.rs` holds all state. Backend results come back tagged with the page and device they are for, so ones for a channel no longer shown are dropped.
- `layout.rs` ports `components/Tiles.tsx`, serialized exactly like the web's so both read and write the same saved layout. Each leaf's `id` is in memory only.
- `feed.rs` ports `components/deviceFeed.ts`. `reconcile` keeps one stream per device some leaf shows, like the reference counted feeds.
- The controls menu is the phone replica's quick buttons in their order, with the web's confirmations and permissions.
- Below 80 columns only the focused pane is shown.
- The message pane takes every key but `tab` and `alt`+arrows, so a click on a pane's title row is the only way to its pane menu.
- A horizontal divider is the upper pane's bottom border, so title rows stay free for dragging.

## Preview images

- `Picker::from_query_stdio` runs once at start, before the key reader, since it reads the terminal's answers from stdin.
- The phone pane only draws the last encoded image (`ResizeEncodeRender::render`, which never encodes) and asks for its size in `wanted`. `App::encode_previews` scales and encodes in a blocking task, so maximizing a pane or a new frame never holds up the screen.
- Dependencies are built optimized even in debug builds (`[profile.dev.package."*"]`), since `image`'s scaling and the zlib compression are many times slower without.
- Encodings alternate between two kitty image ids per device: a new frame under the same id would only rewrite the first placeholder cell, since ratatui redraws only changed cells and kitty keeps the others on the old image. A new id changes every cell's color, and reusing the other id bounds kitty to two images.

## Signing in

- `sign_in.rs` is the native app flow of RFC 8252 with PKCE, so the session never appears in a URL, the browser's history or a log, and the browser is only answered once the code was traded for the session.
- `c` signs in on another device instead, for SSH and headless machines. The address and QR code leave out the code, so it has to be typed.
- `session.rs` keeps the session per backend URL in `sessions.json`, mode 0600.
