#!/bin/sh
set -eu

boolean() {
  case "$2" in
    true|false) printf '%s' "$2" ;;
    *) printf '%s must be true or false\n' "$1" >&2; exit 1 ;;
  esac
}

selfhosted=$(boolean SELF_HOSTED_LANDING_PAGE "${SELF_HOSTED_LANDING_PAGE:-false}")
json_string() {
awk -v variable="$1" 'BEGIN {
  for (i = 1; i < 32; i++) control[sprintf("%c", i)] = i
  value = ENVIRON[variable]
  printf "\""
  for (i = 1; i <= length(value); i++) {
    char = substr(value, i, 1)
    if (char == "\\" || char == "\"") printf "\\%s", char
    else if (char in control) printf "\\u%04x", control[char]
    else printf "%s", char
  }
  printf "\""
}'
}
printf 'window.config = { backendUrl: "/api", selfHostedLandingPage: %s, instanceName: %s };\n' "$selfhosted" "$(json_string INSTANCE_NAME)" > /srv/config.js
exec "$@"
