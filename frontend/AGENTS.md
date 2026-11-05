# Frontend

## Commands

- Build: `npm run build` (runs `tsc --noEmit` first, so type errors fail the build).
- One test file: `npx vitest run src/data/sceneResolver.test.ts`.

Tests are Vitest in Node, `src/**/*.test.ts`, only for the pure code in `src/data/`. `vitest.config.ts` is apart from `vite.config.ts` so the Solid plugin's jsdom is not needed, and `src/testSetup.ts` stubs what `i18n.ts` touches, so `t()` answers English. The scene editor's tests are Moblin's `SceneEditorGeometrySuite` and `SceneResolverSuite` ported case by case.

## Pages

- Multi-page, no router. Adding a page takes three things: the HTML file, its `src/*.tsx` entry and the `rollupOptions.input` line in `vite.config.ts`. The HTML also needs `<script src="/config.js"></script>` before its module.
- Sub pages live in a directory so GitHub Pages serves clean URLs like `/account/`.
- `404.html` is what GitHub Pages serves for unknown paths, so it renders the addresses with a variable part: `/twitch/<login>`, `/kick/<login>`, `/dashboard/<platform>/<login>`, `/device/<page id>/<device id>` and `/monitor/<page id>`. The dev server has the same rewrite in `vite.config.ts`.
- The dashboard is at `/`. `dashboard/index.html` stays for the backend's sign in, PayPal and Stripe redirects and old links. An assisted streamer's dashboard stays under `/dashboard/<platform>/<login>`, since `/<platform>/<login>` is their tip page.
- Dashboard, Monitor, Admin, Account, Connect and the devices are one page (`SignedIn` in `pages/Dashboard.tsx`) switched in place with `history.pushState`. Sentinels like `monitorPage` and `adminPage` are values no `streamerId` can be.
- A link token is only posted with the session stored on that browser, so a link always lands on the account signed in there.
- The Connect code is always typed, never taken from the address, so a link someone sends can't fill it in. `src/dashboard.tsx` marks a visit to `/connect/` in `sessionStorage`, since signing in loses the address.

## Rules

- Keep container runtime options aligned across `runtime-config.sh`, the config type, runtime tests, Compose, `.env.example` and deployment docs. Values in browser config are public; serialize user-supplied strings safely and never put credentials there.
- Landing-page selection and instance naming are presentation only. Do not let them change provider availability, authorization or device protocols. Keep the original page as the default.

- The base path is dynamic: `/` when `public/CNAME` exists and `/moblin_cloud/` otherwise. Anything referencing a file under `public/` must go through `asset()` in `src/asset.ts`; never hardcode a leading slash in a src attribute.
- Content lives in `src/data/`, not in components: URLs in `links.ts`, effects, Moblin's settings and the remote control types in their own files.
- Backend access goes through `data/backend.ts`. The backend is configured at deploy time, not build time: `backendUrl` in `public/config.js`, which the Deploy frontend workflow overwrites in `dist/` since GitHub Pages can't proxy `/api`.
- Server-sent events are read with `subscribe` in `src/sse.ts`, with `fetch` and the bearer session. Change streams only name what changed (`subscribeChanges`) and refetch through the normal GET routes, also when the stream reopens, since events may have been missed while it was closed.
- Theming is Tailwind v4 `@theme` tokens in `src/index.css`. Dark is the default; light overrides the same names. Use the tokens, not raw colors. The `--color-phone-*` and `--color-cushion-*` tokens are the same in both themes, and the `--color-chart-*` ones are validated for both.
- Icons are inline stroke SVGs in `components/Icon.tsx` keyed by `IconName`; platform logos on a colored disc are `BrandIcon`.
- Sign in platforms and payment providers come from `GET /config` (`signInPlatforms` and `paymentProviders` in `data/account.ts`). Sign in offers none until it answers, so an unconfigured one never flashes, and both when it fails. An assistant's `settings` say neither provider is configured, so texts use `paymentProviders`. The admin page goes by its stats' `integrations`.

## Localization

- Wrap every user-visible English string in `t("...")` (placeholders like `{name}` from the second argument) or `tRich("...", { link: <a>...</a> })` for sentences with elements in them.
- The English string is the key and `scripts/translate.py` finds them with a regex, so the first argument must be a double-quoted literal (or literals joined with `+`), never a variable. It also picks up the `<title>` of every HTML entry.
- Run `just machine-translate` after adding or changing strings. Strings without a translation show in English.
- Only the chosen language's `src/locales/<code>.json` is loaded, with top-level await, so `t()` works at module level in `src/data/`.
- The Terms page and the admin page stay English.

## Phone replica

- `MoblinScreen` is a replica of Moblin's landscape UI, drawn at Moblin's point sizes on an 852×393 screen and scaled, built only from what the remote control protocol gives: the device's `state`, `status` and `settings`. There is no chat, and Moblin's other settings are its defaults.
- The quick buttons are the replica's own fixed list (`quickButtons`), since the protocol does not say which buttons the phone shows.
- The preview stream is only opened while it is shown, to save the device's traffic.

## Monitor

- `components/Tiles.tsx` and mobtop's `layout.rs` share one format. `sizes` are exact fractions of the split, and the gaps are each pane's padding with the dividers lying over the boundaries, so splitting or removing a pane never moves its neighbours.
- A leaf keeps its `device` when another panel is picked, so picking a device panel again shows the same device.
- Device panels share one event stream per device through the reference counted `useDeviceFeed` and `usePreviewFeed`. A feed is kept 3 s after its last user goes, so switching a panel or page does not send Moblin `stopStatus` and `startStatus` again.
- `sanitize` checks the saved layout and falls back to the default one.
- A maximized pane only hides the others, so nothing in them reloads.
- On a small screen the tiled layout is `sticky top-0` over a `50dvh` spacer, so a swipe on a pane scrolls the page, which is what a phone browser collapses its bars on. One bar's height of scroll room, as `h-lvh` gives, is not enough for Safari in landscape.
- iPhone Safari only has the Fullscreen API for videos, so `fullscreenOffered` hides full screen where it does nothing. Only leave the mode on `fullscreenchange` when the mode entered it, so a video going full screen in a pane does not end it.

## Admin

- Problems only sums the failure reasons that are our, PayPal's, Stripe's or Moblin's fault. Viewer mistakes and offline streamers are never counted as problems.
- A new failure reason needs an entry in `failures` and `reasonLabels` in `Admin.tsx`.
