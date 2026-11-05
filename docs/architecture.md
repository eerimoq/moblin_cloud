# Architecture and implementation notes

Read the subsystem relevant to the change alongside its source and folder
`AGENTS.md`. Keep the affected note current when behavior or paths change.
Commands live in the `justfile`, setup in [README.md](../README.md), and shared
agent rules in [AGENTS.md](../AGENTS.md).

Moblin Cloud provides streamer dashboards, viewer pages and Moblin device
connections. The separate marketing site lives in `moblin_website`.

- `frontend/`: static SolidJS website.
- `backend/`: Rust API, SQLite storage and device WebSocket service.
- `mobtop/`: terminal client of the same API.
- `deploy/`: Compose and Caddy setup; see [deployment setup](../deploy/README.md).

## Subsystems

- [frontend](architecture/frontend.md)
- [frontend styling and localization](architecture/frontend-styling-and-localization.md)
- [frontend scene editor](architecture/frontend-scene-editor.md)
- [backend api](architecture/backend-api.md)
- [backend subscribers](architecture/backend-subscribers.md)
- [backend assistants](architecture/backend-assistants.md)
- [backend devices](architecture/backend-devices.md)
- [backend storage](architecture/backend-storage.md)
- [backend auth and accounts](architecture/backend-auth-and-accounts.md)
- [backend payments](architecture/backend-payments.md)
- [mobtop](architecture/mobtop.md)
