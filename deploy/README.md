# Deploy

Runs your own copy of the website and its backend on one server with Docker
Compose, from the published images: the backend, and the frontend in Caddy,
which serves the website, proxies `/api` to the backend and gets certificates
from Let's Encrypt.

## Server

Point a domain, for example `example.com`, at the server and open ports 80
and 443. Then, on the server:

```
mkdir moblin-cloud && cd moblin-cloud
curl -O https://raw.githubusercontent.com/eerimoq/moblin_website/main/deploy/docker-compose.yml
curl -o .env https://raw.githubusercontent.com/eerimoq/moblin_website/main/deploy/.env.example
mkdir data
sudo chown 65532:65532 data
```

Fill in `.env`: the domain, and uncomment `SESSION_SECRET` and set it to the
output of `openssl rand -hex 32`, without which everyone is signed out when
the backend restarts. Uncomment the integrations you set up below. Without
`VERSION` the latest images are used.

## Start

```
docker compose up -d
```

Update with `docker compose pull && docker compose up -d`. The database is
`data/database.sqlite`; stop the backend before copying it for a backup.

## Integrations

Everything is optional, but nobody can sign in without Twitch or Kick.

- Twitch: create an application at https://dev.twitch.tv/console with the
  OAuth redirect URL `https://example.com/api/auth/twitch/callback`, and set
  `TWITCH_CLIENT_ID` and `TWITCH_CLIENT_SECRET`.
- Kick: the same at https://kick.com/settings/developer with
  `https://example.com/api/auth/kick/callback`, and `KICK_CLIENT_ID` and
  `KICK_CLIENT_SECRET`.
- Stripe: a Stripe Connect platform account, with `STRIPE_SECRET_KEY` and
  `STRIPE_PUBLISHABLE_KEY`. PayPal needs an approved PayPal partner account,
  so it is left out here.
- `ADMINS`: your Twitch user id, or `kick:<Kick user id>`, for the admin page.

Moblin connects to `https://api.moblin.app`, so a Moblin has to be built with
`MOBLIN_WEBSITE_API_URL` set to `https://example.com/api` to connect to your
copy.
