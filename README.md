# Moblin website

Landing page for Moblin and the rest of the Moblin family: Mobcam, Moblink and
the remote control relays, plus the streamer pages where viewers tip, spin the
wheel and request songs.

- `frontend/` is the website, a static SolidJS + TypeScript + Tailwind site
  deployed to GitHub Pages on every push to `main`.
- `backend/` is a small Rust service deployed at `https://api.moblin.app`.

## Prerequisites

- [Node.js](https://nodejs.org/) and npm
- [Rust](https://rustup.rs/)
- [just](https://github.com/casey/just)
- Python 3 with `pip install -r requirements.txt`
- For the checks: `oxfmt`, `oxlint` and `codespell`

## Develop

Install the frontend dependencies once:

```
cd frontend
npm install
```

Then run the backend and the frontend in two terminals:

```
just backend-run
just frontend-run
```

and open http://localhost:5173. The backend runs in development mode: it is
filled with made-up streamers and signs in with a fake Twitch and a fake Kick
(`tests/utils/fake_twitch.py` and `tests/utils/fake_kick.py`, started by
`just backend-run`), so no credentials are needed. Sign in at
http://localhost:5173/dashboard/ and the fake asks which made-up user to
sign in as (the same login on Twitch and Kick is two different users until
linked in the dashboard's Linked accounts section): `admin` is an admin,
`premium` a premium account, `normal` neither, a login ending in 1, 2 or 3
is a subscriber of that tier, and `mod_normal` moderates `normal` (listed in
the dashboard when `normal` signs in, to be turned on there). Each login has
its own account, so sign out and sign in as another to switch; to have two
signed in at once, use a private window or another browser profile, as a
browser holds one session. To try a streamer page, copy the Moblin Cloud key
and run `just backend-fake-streamer <Moblin Cloud key>` to connect a made-up
Moblin.

## Checks

```
just style
just lint
just spell-check
just test
```

CI runs the same commands. Run `just --list` for the other recipes.
