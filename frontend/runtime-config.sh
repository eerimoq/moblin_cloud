#!/bin/sh
set -eu

boolean() {
  case "$2" in
    true|false) printf '%s' "$2" ;;
    *) printf '%s must be true or false\n' "$1" >&2; exit 1 ;;
  esac
}

selfhosted=$(boolean SELF_HOSTED_LANDING_PAGE "${SELF_HOSTED_LANDING_PAGE:-false}")
tipping=$(boolean TIPPING_ENABLED "${TIPPING_ENABLED:-true}")
printf 'window.config = { backendUrl: "/api", selfHostedLandingPage: %s, tippingEnabled: %s };\n' "$selfhosted" "$tipping" > /srv/config.js
exec "$@"
