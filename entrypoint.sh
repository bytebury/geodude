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
  HTTP_STATUS=$(curl -sSL -o "$TMP/db.zip" -w "%{http_code}" \
    "https://www.ip2location.com/download/?token=${IP2LOCATION_TOKEN}&file=${IP2LOCATION_FILE_CODE}")
  echo "HTTP status: $HTTP_STATUS, file size: $(stat -c%s "$TMP/db.zip" 2>/dev/null || echo '?') bytes"

  # IP2Location returns 200 with a plaintext/HTML error body when the request is rejected
  # (bad token, invalid file code, quota exceeded). Detect that before extracting.
  if ! unzip -tq "$TMP/db.zip" >/dev/null 2>&1; then
    echo "Downloaded file is not a valid zip. First 500 bytes of response:" >&2
    head -c 500 "$TMP/db.zip" >&2
    echo >&2
    echo "Check IP2LOCATION_TOKEN, IP2LOCATION_FILE_CODE, and your daily download quota." >&2
    exit 1
  fi

  unzip -o "$TMP/db.zip" "*.BIN" -d "$TMP" >/dev/null
  # Some archives ship both IPv4 and IPv6 BINs; prefer the IPv6 (combined) variant when present.
  SRC=$(ls "$TMP"/*IPV6*.BIN 2>/dev/null | head -n1)
  if [ -z "$SRC" ]; then
    SRC=$(ls "$TMP"/*.BIN | head -n1)
  fi
  mv "$SRC" "$BIN_PATH"
  rm -rf "$TMP"
  echo "Database written to $BIN_PATH ($(stat -c%s "$BIN_PATH") bytes)"
fi

exec "$@"
