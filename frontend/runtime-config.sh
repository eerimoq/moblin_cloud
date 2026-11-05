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
instance_name=$(awk 'BEGIN {
  for (i = 1; i < 32; i++) control[sprintf("%c", i)] = i
  value = ENVIRON["INSTANCE_NAME"]
  printf "\""
  for (i = 1; i <= length(value); i++) {
    char = substr(value, i, 1)
    if (char == "\\" || char == "\"") printf "\\%s", char
    else if (char in control) printf "\\u%04x", control[char]
    else printf "%s", char
  }
  printf "\""
}')
printf 'window.config = { backendUrl: "/api", selfHostedLandingPage: %s, tippingEnabled: %s, instanceName: %s };\n' "$selfhosted" "$tipping" "$instance_name" > /srv/config.js
exec "$@"
