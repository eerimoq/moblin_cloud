#!/bin/sh
set -eu

cat <<EOF > /srv/config.js
window.config = {
  backendUrl: "${BACKEND_URL:-/api}",
  landingPage: "${LANDING_PAGE:-default}",
  instanceName: "${INSTANCE_NAME:-}",
};
EOF

exec "$@"
