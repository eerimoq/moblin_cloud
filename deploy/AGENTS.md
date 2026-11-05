# Deploy

Setting up and running a copy is in `README.md`.

- Keep the public-viewer flag identical on frontend and backend. A hidden browser control is not an API restriction; verify the backend gate and the device/payment exemptions, and update Compose, `.env.example`, runtime tests and docs together.

- Moblin's WebSocket and the pages' server-sent events go through the `caddy` service and `frontend/Caddyfile`, so a routing change has to keep WebSocket upgrades working and must not buffer responses.
- The database is in WAL mode, so the latest writes may only be in `database.sqlite-wal`. That is why a backup needs the backend stopped.
- `.env` is a real copy's secrets and is ignored by git. What others should see goes in `.env.example`.
