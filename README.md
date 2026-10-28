# Moblin Cloud

The streamer pages where viewers tip and spin the wheel, and the dashboard
where streamers manage them and their Moblin devices.

- `frontend/` is the site (SolidJS, TypeScript and Tailwind), deployed to
  GitHub Pages on every push to `main`.
- `backend/` is a Rust service deployed at `https://cloud-api.moblin.app`.
- `deploy/` runs your own copy of both on one server, see
  [deploy/README.md](deploy/README.md).

## Prerequisites

Node.js, Rust, [just](https://github.com/casey/just), Python 3 with
`pip install -r requirements.txt`, and `oxfmt`, `oxlint` and `codespell` for
the checks.

## Develop

```
cd frontend && npm install
just dev
```

and open http://localhost:5173. It runs the backend, the frontend and a fake
Moblin. Signing in goes through a fake Twitch and Kick that
take any login: `admin` is an admin, `premium` a premium account and a login
ending in 1, 2 or 3 a subscriber. Run `just` for the other recipes.

## Checks

```
just style
just lint
just spell-check
just test
```

CI runs the same commands.
