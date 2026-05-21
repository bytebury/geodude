#!/bin/sh
set -e

BIN_PATH="${IP2LOCATION_BIN_PATH:-/data/ip2location.BIN}"
MAX_AGE_DAYS="${IP2LOCATION_MAX_AGE_DAYS:-30}"
REFRESH_SCRIPT="${IP2LOCATION_REFRESH_SCRIPT:-/app/refresh-db.sh}"
mkdir -p "$(dirname "$BIN_PATH")"

# Maintenance mode keeps the container alive without launching the app.
# Use it to `railway ssh` in and copy a BIN onto a freshly-attached volume.
if [ -n "$MAINTENANCE_MODE" ]; then
  echo "MAINTENANCE_MODE set — sleeping. ssh in and populate $BIN_PATH, then unset and redeploy."
  exec sleep infinity
fi

REFRESH_REASON=""
if [ ! -f "$BIN_PATH" ]; then
  REFRESH_REASON="missing"
elif [ -n "$(find "$BIN_PATH" -mtime +"$MAX_AGE_DAYS" -print 2>/dev/null)" ]; then
  REFRESH_REASON="older than ${MAX_AGE_DAYS} days"
fi

if [ -n "$REFRESH_REASON" ]; then
  if [ -z "$IP2LOCATION_TOKEN" ] || [ -z "$IP2LOCATION_FILE_CODE" ]; then
    if [ -f "$BIN_PATH" ]; then
      echo "BIN $REFRESH_REASON but IP2LOCATION_TOKEN/IP2LOCATION_FILE_CODE not set; continuing with existing file." >&2
    else
      echo "IP2LOCATION_TOKEN and IP2LOCATION_FILE_CODE must be set to download the database." >&2
      exit 1
    fi
  else
    echo "Refresh triggered on boot: $REFRESH_REASON"
    if ! "$REFRESH_SCRIPT"; then
      if [ -f "$BIN_PATH" ]; then
        echo "Refresh failed; continuing with existing BIN ($REFRESH_REASON)." >&2
      else
        exit 1
      fi
    fi
  fi
fi

exec "$@"
