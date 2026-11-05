# Deployment configuration

Read the root `AGENTS.md` and [deploy/README.md](README.md).
Keep these instructions portable and grounded in the tracked Compose setup.

`docker-compose.yml` runs the backend, the static frontend in Caddy and a TLS
Caddy proxy. The frontend proxies `/api` to `BACKEND_HOST`. Routing changes must
preserve WebSocket upgrades and unbuffered server-sent events.

Treat `.env` as secret-bearing. Edit the tracked example for public documentation,
not a real instance's configuration. Keep OAuth credentials, `SESSION_SECRET`
and device keys out of browser settings, logs, images and source control.
A persistent session secret preserves browser sessions across backend restarts.

Use matching frontend/backend versions when changing their shared contract.
Explain whether a setting is read during build or process startup. Check the
current Compose file before claiming that a setting exists or is passed to a
service. Do not document settings from an unmerged branch as current behavior.

SQLite uses WAL. A plain file-copy backup needs the writer stopped or a supported
SQLite snapshot mechanism. Restore to an isolated copy and verify integrity
before operating on the live database.

For Compose edits, validate interpolation/defaults without exposing secrets.
For image/proxy edits, build the affected image and check the relevant routing
and startup behavior. Local verification does not authorize publication or a
production configuration change. Update `deploy/README.md` in the same change
when operator-facing setup or recovery steps change.
