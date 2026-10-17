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
filled with made-up streamers and signing in with Twitch is faked, so no
credentials are needed. To try a streamer page, sign in at
http://localhost:5173/dashboard/, copy the Moblin Cloud key and run
`just backend-fake-streamer <Moblin Cloud key>` to connect a made-up Moblin.

## Checks

```
just style
just lint
just spell-check
just test
```

CI runs the same commands. Run `just --list` for the other recipes.
