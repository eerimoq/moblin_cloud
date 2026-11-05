# frontend styling and localization

[Architecture index](../architecture.md). Keep this note current with its implementation.

**Theming** is Tailwind v4 `@theme` tokens in `src/index.css` (e.g. `bg-card`,
`text-leaf`, `font-display`). Dark is the default; light is the same token names
overridden under `prefers-color-scheme: light`. Use the tokens, not raw colors.

**Localization** is `src/i18n.ts`: wrap every user-visible English string in `t("...")`
(placeholders like `{name}` filled from the second argument) or `tRich("...", { link:
<a>...</a> })` for sentences with elements in them. The English string is the key, and
`scripts/translate.py` (`just machine-translate`, like Moblin's) finds them with a
regex, so the first argument must be a double-quoted literal (or literals joined with
`+`), never a variable. It also picks up the `<title>` of every HTML entry, which
`i18n.ts` translates. Translations live in `src/locales/<code>.json`, one per language,
and only the chosen one is loaded, with top-level await before any module using `t()`
runs, so `t()` works at module level in `src/data/`. The language comes from the
footer's picker (saved in `localStorage`, reloads the page) or the browser. Strings
without a translation show in English. See [translation
workflow](../../scripts/AGENTS.md). The Terms page, the admin page and backend error
messages stay English.

Icons are inline stroke SVGs in `components/Icon.tsx` keyed by `IconName`; platform
logos on a colored disc are `BrandIcon`.
