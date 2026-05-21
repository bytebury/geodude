#!/bin/bash
set -e

trap "kill 0" EXIT

# Install cargo-watch if not present
if ! command -v cargo-watch &> /dev/null
then
  echo "cargo watch not found. Installing..."
  cargo install cargo-watch
fi

# Check to see if the user set up an environment file
if [ ! -f .env ]; then
  echo "🤖 .env not found. Generating..."
  cat >.env <<EOF
PORT=8080
IP2LOCATION_BIN_PATH=~/home/marcello/Downloads/IP2LOCATION-LITE-DB3.IPV6.BIN/IP2LOCATION-LITE-DB3.IPV6.BIN
EOF
  echo "✅ .env generated."
else
  echo "✅ .env file found."
fi

# Start the dev server
echo "🦀 Starting Rust dev server..."
cargo watch -x 'run'