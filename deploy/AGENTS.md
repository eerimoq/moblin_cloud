# Portable deployment

Read the root `AGENTS.md` and this folder's `README.md`. Keep these instructions
portable across self-hosting platforms. Host-specific provisioning belongs in the
operator's infrastructure repository, not this upstream application.

Compose runs the backend, the static frontend in Caddy, and an outer TLS Caddy
when needed. The frontend proxies `/api` to `BACKEND_HOST`; reverse proxies must
support WebSocket upgrades and unbuffered SSE responses.

Treat `.env` as secret-bearing. Edit only tracked examples for documentation.
Keep OAuth credentials, `SESSION_SECRET` and device keys out of frontend runtime
settings, images, logs and source control. A persistent session secret is needed
to preserve sessions across backend restarts.

Use matching reviewed frontend/backend image revisions. Explain whether a
setting is read at build time, container startup or backend startup. Changing
`.env` does not update an existing container until it is recreated.

Presentation settings do not control access. Effective feature settings must
agree between clients and the backend. `/config.js` and `/api/config` must be usable by the signed-out
frontend. Do not put phone/terminal APIs behind a browser-only access gate.

Protect SQLite backup consistency: stop the writer for a plain file copy or use
a supported SQLite snapshot mechanism. Keep WAL behavior in mind. Restore to an
isolated copy and verify integrity and application behavior before touching the
live volume.

Validate Compose interpolation and defaults, build affected images, and check
the generated public config. Test both external TLS termination and the supplied
TLS proxy when changing routing. Do not publish images or modify production as
part of a documentation or local-validation task.
