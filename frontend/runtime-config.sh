#!/bin/sh
set -eu

boolean() {
  case "$2" in
    true|false) printf '%s' "$2" ;;
    *) printf '%s must be true or false\n' "$1" >&2; exit 1 ;;
  esac
}

selfhosted=$(boolean SELFHOSTED "${SELFHOSTED:-false}")
details=$(boolean LANDING_DETAILS_ENABLED "${LANDING_DETAILS_ENABLED:-true}")
tipping=$(boolean TIPPING_ENABLED "${TIPPING_ENABLED:-true}")
printf 'window.config = { backendUrl: "/api", selfHosted: %s, landingDetailsEnabled: %s, tippingEnabled: %s };\n' "$selfhosted" "$details" "$tipping" > /srv/config.js
exec "$@"
