# Deploy

Runs your own copy of Moblin Cloud and its backend on one server with Docker
Compose, from the published images: the backend, the frontend in Caddy,
which serves the site on plain HTTP and proxies `/api` to the backend at
`BACKEND_HOST` (`backend:8080` by default), and
in front of them the stock Caddy image, which gets the certificate from Let's
Encrypt and serves HTTPS.

## Server

Point a domain, for example `example.com`, at the server and open ports 80
and 443. Then, on the server:

```
mkdir moblin-cloud && cd moblin-cloud
curl -O https://raw.githubusercontent.com/eerimoq/moblin_cloud/main/deploy/docker-compose.yml
curl -o .env https://raw.githubusercontent.com/eerimoq/moblin_cloud/main/deploy/.env.example
mkdir data
sudo chown 65532:65532 data
```

Fill in `.env`: the domain, and uncomment `SESSION_SECRET` and set it to the
output of `openssl rand -hex 32`, without which everyone is signed out when
the backend restarts. Uncomment the integrations you set up below. Without
`VERSION` the latest images are used.

## HTTPS

The `caddy` service terminates TLS: on the first start it gets a certificate
for `DOMAIN` from Let's Encrypt, which needs ports 80 and 443 reachable from
the internet, and then renews it, redirects HTTP to HTTPS and forwards
everything to the frontend. The certificates are kept in the `caddy` volume.

To terminate TLS in a reverse proxy already running on the server instead,
remove the `caddy` service, publish the frontend with `ports: ["127.0.0.1:8080:80"]`
and forward the domain to `http://127.0.0.1:8080`. The proxy has to pass
WebSocket upgrades (Moblin connects to `/api/remote-control/streamer/`) and
must not buffer responses, since the dashboard, the tip page and the device
page are server-sent event streams.

## Start

```
docker compose up -d
```

Update with `docker compose pull && docker compose up -d`. The database is
`data/database.sqlite`; stop the backend before copying it for a backup.

## Self-hosted landing page

Set `SELF_HOSTED_LANDING_PAGE=true` on the frontend container to select a compact
device-management landing page at `/`. Default: `false`, keeping the original
landing page. The compact page lives in its own component, clearly labeled
“Self-hosted instance”, with linked attribution to Moblin Cloud.

Optionally set `INSTANCE_NAME` to show “Moblin Cloud — name” in the shared header
and compact card. Empty or whitespace-only names keep “Moblin Cloud”. The mascot
stays unchanged; names are public text, not secrets. The original landing ignores
the name. Browser titles, emails, API responses and viewer-page branding stay
unchanged.

```dotenv
SELF_HOSTED_LANDING_PAGE=true
INSTANCE_NAME=My workspace
```

Compose passes both values to the frontend. Recreate it with
`docker compose up -d` after editing `.env`, then reload the browser. No image
rebuild is needed. Use literal `true` or `false` for the landing flag; other
nonempty values are rejected. Local Vite config uses `selfHostedLandingPage`
and `instanceName` in `frontend/public/config.js`.

The landing flag only changes presentation. It does not restrict sign-in,
configure OAuth, grant admin/premium rights or alter tipping and device APIs.
Twitch/Kick buttons use the backend's existing `/api/config` availability. Keep
that endpoint reachable when signed out. If no providers are configured, the
page explains that an administrator must finish setup.

## Integrations

Everything is optional, but nobody can sign in without Twitch or Kick.

- Twitch: create an application at https://dev.twitch.tv/console with the
  OAuth redirect URL `https://example.com/api/auth/twitch/callback`, and set
  `TWITCH_CLIENT_ID` and `TWITCH_CLIENT_SECRET`.
- Kick: the same at https://kick.com/settings/developer with
  `https://example.com/api/auth/kick/callback`, and `KICK_CLIENT_ID` and
  `KICK_CLIENT_SECRET`.
- PayPal: a PayPal partner REST app, with `PAYPAL_CLIENT_ID`,
  `PAYPAL_CLIENT_SECRET` and the partner's merchant ID as `PAYPAL_PARTNER_ID`;
  `PAYPAL_ATTRIBUTION_ID` is the partner's BN code. Subscribe a webhook at
  `https://example.com/api/paypal/webhooks` to `MERCHANT.ONBOARDING.COMPLETED`
  and `MERCHANT.PARTNER-CONSENT.REVOKED` and set `PAYPAL_WEBHOOK_ID` to its ID.
  `PAYPAL_SANDBOX=true` uses PayPal's sandbox.
- Stripe: a Stripe Connect platform, with `STRIPE_SECRET_KEY` and
  `STRIPE_PUBLISHABLE_KEY`. A test mode key takes test payments.
- `ADMINS` and `PREMIUM_ACCOUNTS`: comma separated `twitch:<Twitch user ID>`
  or `kick:<Kick user ID>` (a bare ID is a Twitch one) of the accounts that see
  the admin page and of those that get the premium features. Admins are premium
  too.

Every setting is an environment variable in `.env`; the backend lists them
all with their defaults:

```
docker compose run --rm backend --help
```

Moblin connects to `https://cloud-api.moblin.app`, so a Moblin has to be built
with `MOBLIN_CLOUD_API_URL` set to `https://example.com/api` to connect to your
copy, and `APP_ID` set to that build's app ID.
