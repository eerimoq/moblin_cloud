#!/bin/sh
set -eu

boolean() {
  case "$2" in
    true|false) printf '%s' "$2" ;;
    *) printf '%s must be true or false\n' "$1" >&2; exit 1 ;;
  esac
}

twitch=$(boolean TWITCH_SIGN_IN_ENABLED "${TWITCH_SIGN_IN_ENABLED:-true}")
kick=$(boolean KICK_SIGN_IN_ENABLED "${KICK_SIGN_IN_ENABLED:-true}")
details=$(boolean LANDING_DETAILS_ENABLED "${LANDING_DETAILS_ENABLED:-true}")
tipping=$(boolean TIPPING_ENABLED "${TIPPING_ENABLED:-true}")
printf 'window.config = { backendUrl: "/api", twitchSignInEnabled: %s, kickSignInEnabled: %s, landingDetailsEnabled: %s, tippingEnabled: %s };\n' "$twitch" "$kick" "$details" "$tipping" > /srv/config.js
exec "$@"
