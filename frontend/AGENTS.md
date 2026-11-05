# Frontend

Read the root `AGENTS.md` first. Paths below are relative to `frontend/`.

## Find the implementation

| Location | Owns |
| --- | --- |
| HTML entries, `src/*.tsx`, `vite.config.ts` | Page bootstrapping, base path, build inputs and development rewrites. |
| `src/not-found.tsx` | Static-host fallback for friendly viewer, dashboard, device and monitor URLs. |
| `src/pages/Dashboard.tsx` | Signed-in navigation, selected page/streamer/device and history. |
| `src/components/CloudLanding.tsx` | Landing page. |
| `src/data/account.ts`, `src/data/backend.ts` | API calls, sessions and configured sign-in/payment providers. |
| `src/sse.ts`, `src/components/deviceFeed.ts` | Streaming lifecycle, reconnection and shared device/preview feeds. |
| `src/components/sceneEditor/`, `src/data/moblinSettings.ts` | Ports of Moblin editing and settings behavior. |
| `src/index.css`, `src/i18n.ts`, `src/locales/` | Tokens, translation lookup and locale catalogs. |

The [architecture index](../docs/architecture.md) has separate frontend, scene
editor and styling/localization notes.

## Preserve the client contract

This is a static multi-page SolidJS app, not React or a router-based SPA. When
adding a page, update the HTML entry, its bootstrap and Vite input list together.
Use existing asset/base-path helpers. Check direct navigation and reloads, not
only links from the dashboard.

Preserve Solid reactivity and ownership. Do not snapshot reactive props
accidentally or leak timers/listeners when components disappear. Reuse existing
controls, design tokens and responsive patterns.

Read available integrations through the existing account helpers and `/config`.
The current compatibility fallback offers providers if configuration loading
fails; changing that policy requires a deliberate behavior change and tests.
Never expose credentials or infer authorization from client-side state.

Use the existing fetch-based SSE helper so bearer headers, error handling,
visibility and reconnect behavior remain consistent. Reuse shared device feeds
instead of opening one connection per monitor pane. Preserve unauthorized-session
handling and cleanup.

Use literal strings with `t`/`tRich`, keeping placeholders intact. The translation
workflow is in [scripts/AGENTS.md](../scripts/AGENTS.md); do not duplicate its rules
here. Keep scene-editor changes aligned with the Swift source and port map.

## Check the change

Run `npm test` for logic changes and `npm run build` for TypeScript and production
output. Use the root check recipes for the applicable lint/style checks.

Vitest runs in Node with small document/storage stubs, not a browser. For UI
changes, check relevant desktop/narrow-screen states when browser tools are
available. Exercise configured/unconfigured providers, loading/failure, signed-in
and signed-out states as applicable. Use fake services rather than real accounts
for ordinary UI checks. Update the owning architecture note when behavior moves.
