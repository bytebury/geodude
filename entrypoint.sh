#!/bin/sh
set -e

BIN_PATH="${IP2LOCATION_BIN_PATH:-/data/ip2location.BIN}"
mkdir -p "$(dirname "$BIN_PATH")"

if [ ! -f "$BIN_PATH" ]; then
  if [ -z "$IP2LOCATION_TOKEN" ] || [ -z "$IP2LOCATION_FILE_CODE" ]; then
    echo "IP2LOCATION_TOKEN and IP2LOCATION_FILE_CODE must be set to download the database." >&2
    exit 1
  fi
  echo "Downloading IP2Location database ($IP2LOCATION_FILE_CODE)..."
  TMP=$(mktemp -d)
  curl -fsSL -o "$TMP/db.zip" \
    "https://www.ip2location.com/download/?token=${IP2LOCATION_TOKEN}&file=${IP2LOCATION_FILE_CODE}"
  unzip -o "$TMP/db.zip" "*.BIN" -d "$TMP" >/dev/null
  mv "$TMP"/*.BIN "$BIN_PATH"
  rm -rf "$TMP"
  echo "Database written to $BIN_PATH"
fi

exec "$@"
