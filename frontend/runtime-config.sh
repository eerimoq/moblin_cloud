#!/bin/sh
set -eu
case "${TIPPING_ENABLED:-true}" in
  true|false) tipping=${TIPPING_ENABLED:-true} ;;
  *) printf 'TIPPING_ENABLED must be true or false\n' >&2; exit 1 ;;
esac
printf 'window.config = { backendUrl: "/api", tippingEnabled: %s };\n' "$tipping" > /srv/config.js
exec "$@"
