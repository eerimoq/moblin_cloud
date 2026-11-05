# backend storage

[Architecture index](../architecture.md). Keep this note current with its implementation.

`database.rs` — the SQLite database (`rusqlite`, SQLite bundled) at `--database`, in
memory without it, shared by `Accounts`, `Pages`, `Orders`, `Deliveries` and `Counters`
behind one mutex. Tables are created with `CREATE TABLE IF NOT EXISTS` on start and
there are no migrations, so a schema change needs the existing database changed by hand.

`counters.rs` — event counters for the admin page, one row per name and UTC day in the
`counters` table, read back as today, last 7 days and all time. Measurement points come
in pairs, `<name>` for successes and `<name>Failed:<reason>` for failures, where the
reason is the `reason` slug every `Failure` in the API carries (`counted` does the
bumping), which the admin page lists as one plain row per reason under the success row,
always showing the reasons each code path can produce (`failures` per event in
`Admin.tsx`, labels in `reasonLabels`) plus any unexpected one that occurred: created
orders and captured payments (`tipOrders`, `tipPayments` and so on per `Item::kind`, any
refusal or provider error counting as failed), the same per provider (`paypalOrders`,
`stripeOrders`, `paypalPayments`, `stripePayments` by `Payee::slug`: orders from the
moment the provider is asked to create one in `start_order`, captures from the moment it
is asked to capture in `capture_at_provider`, so the page's own refusals before that
only count under the kind; a captured payment counts for its kind and its provider in
the same transaction), spins of subscribers (`subSpins`), the dashboard's free test
sends (`tipTests`, `spinTests`; their deliveries and expiries are counted as
`tipTestDeliveries`, `tipTestExpiries` and so on, shown in their own Tests table and
left out of the Problems), and in `deliveries.rs` a delivery that reached every device
(`tipDeliveries`, `spinDeliveries`) or that Moblin refused (`...DeliveriesFailed`, once
per delivery and reason). The free `counters::bump` takes a connection, for callers
already holding the database lock.

`pages.rs` — per streamer page settings (`Pages`), keyed by the public id: the one
currency of all its payments (USD by default, set in the dashboard's Payments section;
only changed while tips, the wheel and effects are all off, refused with 409 otherwise,
so their amounts never silently switch currency; turning one on in the same save as the
currency is fine, and the dashboard disables the picker while one is on), optional tips
(one to five suggested amounts in hundredths), optional effects (one price), an optional
spin wheel (one to five options of a number of spins and its price) and the subscriber
tiers, each a `Feature` with an `enabled` flag next to its values, so turning one off
keeps its values for when it is turned on again (`enabled` defaults to true, so settings
saved before the flag are on; the checks and the public answers only see features that
are on, through `Settings::tips()` and the like, and the public answers show one that is
off as null) and the look of the page (message up to 300 characters, accent color from a
fixed list whose CSS lives in `index.css` as `[data-accent]` overrides of the moss/leaf
tokens, or for premium accounts any `#rrggbb` color, drawn as `[data-accent="custom"]`
with the color in `--accent` (`accentAttributes`; a streamer taken off
`--premium-accounts` keeps theirs but can't pick a new one), and, for premium accounts,
a font from a fixed list (refused with 403 for others, who likewise keep one picked
while premium), `[data-font]` overrides of `--font-display`/`--font-body` whose Google
Fonts `loadFonts` adds only when used). `GET /pages/{id}` returns the look with the
name, avatar and banner image version (`images.rs`). Saved as JSON in the `pages` table
on every change.
