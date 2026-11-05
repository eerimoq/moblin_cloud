#!/bin/sh
set -eu

boolean() {
  case "$2" in
    true|false) printf '%s' "$2" ;;
    *) printf '%s must be true or false\n' "$1" >&2; exit 1 ;;
  esac
}

selfhosted=$(boolean SELF_HOSTED "${SELF_HOSTED:-false}")
tipping=$(boolean TIPPING_ENABLED "${TIPPING_ENABLED:-true}")
printf 'window.config = { backendUrl: "/api", selfHosted: %s, tippingEnabled: %s };\n' "$selfhosted" "$tipping" > /srv/config.js
exec "$@"
