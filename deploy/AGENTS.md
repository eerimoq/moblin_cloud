# Deploy

Setting up and running a copy is in `README.md`.

- Runtime frontend configuration is created when the container starts. Changing an environment value requires container recreation and a browser reload, not an image rebuild; keep the documented defaults and runtime tests synchronized.

- Moblin's WebSocket and the pages' server-sent events go through the `caddy` service and `frontend/Caddyfile`, so a routing change has to keep WebSocket upgrades working and must not buffer responses.
- The database is in WAL mode, so the latest writes may only be in `database.sqlite-wal`. That is why a backup needs the backend stopped.
- `.env` is a real copy's secrets and is ignored by git. What others should see goes in `.env.example`.
