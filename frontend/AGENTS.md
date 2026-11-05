# Frontend

Read the repository's `AGENTS.md` first. This is a static SolidJS, TypeScript,
Tailwind v4 and Vite application with multiple HTML entries.

## Ownership

- HTML entries and `src/*.tsx` bootstraps select the page; `vite.config.ts` lists
  the build inputs. Update all three when adding a page.
- `src/not-found.tsx` handles friendly viewer, dashboard, device and monitor URLs
  through the static host's 404 fallback. Test those direct URLs and reloads.
- `src/pages/Dashboard.tsx` coordinates signed-in navigation, selected streamer
  and device. Preserve history, hash selection and assistance boundaries.
- `src/components/CloudLanding.tsx` owns the landing page.
- `src/data/` owns shared API contracts and transformations. Reuse the existing
  backend call helpers and `signInPlatforms`/`paymentProviders` from `account.ts`.
- `src/sse.ts` owns reconnect/visibility behavior; `components/deviceFeed.ts`
  shares device and preview feeds. Do not create a stream per pane accidentally.
- `components/sceneEditor/` and `data/moblinSettings.ts` mirror Moblin behavior.
  Check the port map and the corresponding Swift code before changing semantics.
- `src/index.css` contains shared design tokens. `src/locales/` contains translated
  strings; `src/i18n.ts` provides `t` and `tRich`.

## Rules

Use Solid signals, resources and cleanup correctly; do not apply React hook or
state assumptions. Preserve the multi-page structure and use existing controls,
tokens, asset helpers and responsive behavior.

Use literal strings with `t`/`tRich` for new user-facing copy and preserve message
placeholders. Do not silently replace locale catalogs with machine output.

Never send provider credentials to the browser. Runtime `config.js` is public.
Do not infer privileges from hidden buttons or client-side role checks. Keep
backend configuration error behavior explicit and test it before changing the
upstream compatibility fallback.

Preserve session invalidation handling and cancel timers, listeners and streams
when owners disappear. Handle 401/403, network failure, reconnect and hidden-tab
behavior. Validate an event-stream response before treating it as connected.

Keep presentation choices independent of backend capabilities. Exercise
Twitch-only, Kick-only, both and neither provider states, missing payment
integrations and direct viewer URLs when changing those flows.

## Verification

```sh
npm test
npm run build
```

Run the repository's frontend lint/style checks as applicable. Check actual
desktop and phone-sized layouts for UI changes, including keyboard focus,
loading, missing integrations and error messages. Do not use real OAuth or
payment credentials for ordinary visual tests.
