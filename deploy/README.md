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

## Self-hosted options

Use matching frontend and backend images. Set these options in `.env` and run
`docker compose up -d` to recreate the containers; no image rebuild is needed.

| Setting | Default | Behavior |
| --- | --- | --- |
| `SELF_HOSTED` | `false` | A compact device-management landing page instead of the original landing page. |
| `TIPPING_ENABLED` | `true` | Public viewer pages and new tip, spin and effect orders. |

For a device-management instance without public viewer pages:

```dotenv
SELF_HOSTED=true
TIPPING_ENABLED=false
```

`SELF_HOSTED` only changes presentation. It does not restrict sign-in or grant
premium/admin access. Disabling tipping hides viewer/share links and returns 404
for public viewer APIs and new orders. Device connections, dashboard APIs,
page-address discovery, existing payment captures and webhooks remain available.
Already open viewer pages may remain visible until a request or reload.

Twitch and Kick sign-in buttons follow the backend's configured client ID and
secret pairs through `/api/config`. Configure either or both providers below;
with neither, the page explains that sign-in needs setup. Missing payment
credentials alone do not disable public viewer pages.

With custom Compose, pass `SELF_HOSTED` to the frontend and `TIPPING_ENABLED` to
both services. Keep OAuth credentials and `SESSION_SECRET` on the backend only.
Frontend flags are public booleans in `/config.js`; for Vite development, use
`selfHosted` and `tippingEnabled` in `frontend/public/config.js`.

Keep `/config.js` and `/api/config` reachable when signed out. If buttons are
missing, check provider credentials and proxy redirects. Browser login gates
must not intercept the phone's WebSocket or event-stream connections. Complete
a real sign-in to verify credentials; availability flags do not validate them.

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

## Connect Moblin

If your Moblin build has a server URL field in its Moblin Cloud settings, set it
to `https://example.com/api` without a trailing slash, enter the Cloud key from
your dashboard and enable the connection. Moblin converts the URL to a WebSocket
URL and appends its connection path; do not add that path or the key to the URL.
The compact landing page and disabled tipping do not change this connection.

For a build without that setting, build Moblin with `MOBLIN_CLOUD_API_URL` set to
`https://example.com/api`. Set the backend's `APP_ID` to that custom build's app ID
when configuring app attestation.

After setup, verify the expected sign-in buttons, complete provider sign-in and
confirm the phone connects. To troubleshoot missing buttons, this public request
should return JSON booleans rather than HTML or a login redirect:

```sh
curl --fail https://example.com/api/config
```
