# Stream settings

A copy of Moblin's stream settings (`StreamsSettingsView.swift`, everything under `Settings/Streams/Stream/` and the create stream wizard) in the phone replica's Settings panel. `scripts/moblin_ports.json` says what each file ports, on Moblin's `remote-control-streams` branch; see `just moblin-drift` in the root `AGENTS.md`.

- The list is the device's settings' `streams` (ids and names), and a stream's full settings are fetched with `getStream` when it is opened or duplicated, so the list needs no request of its own. A stream refetched while it has unsynced edits keeps them.
- It edits a local draft of the opened streams (`store.ts`) and syncs it like the scene editor: after a change, then at most once a second, only what differs, one request at a time, retrying a failure after 5 s.
- Deletes are sent first, so a new stream can take a deleted one's name. A request Moblin refuses only affects its own stream: a refused update refetches it, a refused create drops it and a refused delete brings its row back. Only failures to reach Moblin retry the whole sync.
- Whether a stream is in use, which most live and recording rules depend on, comes from `currentStreamId`, not the fetched stream's `enabled`, which goes stale when the phone switches streams.
- `updateStream` replaces the whole stream and Moblin applies each changed field with the side effects and refusals of its own views, so the forms only edit the draft. Moblin answers `error` for a change its view would have disabled (most while live or recording) and the backend turns it into 409.
- Moblin keeps its own values of what the phone owns, whatever the request says: whether the stream is enabled, the Twitch, Kick and YouTube logins and tokens, the Kick channel ids it looks up, the recording folder and the replay stinger files. `getStream` never carries the access tokens. Stream URLs, passwords, push keys, webhooks and the URLs Moblin sends a secret to (OBS WebSocket, RealtimeIRL, the alerts AI service) only reach the owner and assistants with the Secrets permission; for the others Moblin leaves them out (the URL keeps only its scheme, so the pages still know the protocol) and keeps its own values on updates (see `backend/src/api/AGENTS.md`), so their rows show Hidden and they can't create or duplicate streams.
- Assistants with only `view` access read the streams but change nothing: the replica provides `ReadOnly` from `Form.tsx`, which the form rows and the pages' own controls disable themselves for while navigation stays usable, and the store ignores edits.
- The enabled stream is changed with `setStream`, never through `enabled`, and `getSettings`' `currentStreamId` names it. A stream created from the wizard is selected once it is synced, like Moblin's wizard.
- Multi-streaming destinations have no ids in Moblin's JSON, so `normalizeStream` derives them from the index and `encodeStream` leaves them out.
- `normalizeStream` fills missing keys with Moblin's decode defaults and keeps any field it does not know, so a newer Moblin's settings survive an update.

Left out on purpose:

- Signing in and out of Twitch, Kick and YouTube (OAuth on the phone), and what needs a signed in platform API: the token expiry, the live title and category, YouTube's Manage streams and Fetch Video IDs, and the wizard's stream key and ingest lookups.
- Picking files: the recording folder and the replay stinger videos are picked on the phone.
- Reordering streams, since the protocol has no request for it.
- Global settings Moblin links to from these views: bitrate presets, the remote control assistant and the numeric input toggle.
