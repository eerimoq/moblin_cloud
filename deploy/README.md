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

## Public pages

The frontend container reads these optional settings at startup. All default to
`true`, preserving the existing pages. Set a value to `false` in `.env` and
recreate the affected containers with `docker compose up -d`; no frontend image
rebuild is needed.

- `TWITCH_SIGN_IN_ENABLED` and `KICK_SIGN_IN_ENABLED`: show or hide each
  provider's sign-in buttons, including the header and account-link prompts.
  Match them to the providers configured below. These are presentation settings;
  they do not disable OAuth endpoints or replace backend authentication.
- `LANDING_DETAILS_ENABLED`: show or hide the landing page's How it works section.
- `TIPPING_ENABLED`: enable or disable public viewer pages and new tip, spin and
  effect orders. Compose passes this setting to both the frontend and backend.
  When disabled, the frontend hides viewer pages and share links, and the backend
  returns 404 for public viewer APIs. Phone connections, dashboard APIs,
  page-address discovery, payment webhooks and capture of existing orders remain
  available. Existing open pages may remain visible until a request or reload.

Missing payment credentials do not disable the whole viewer page, since
non-payment interactions can still be used. `TIPPING_ENABLED=false` explicitly
disables it even when payment providers are configured.

Frontend settings are public booleans in `/config.js`, served without caching.
Do not pass provider secrets to the frontend. For local Vite development, use
`twitchSignInEnabled`, `kickSignInEnabled`, `landingDetailsEnabled` and
`tippingEnabled` in `frontend/public/config.js`; omitted fields default to true.

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
