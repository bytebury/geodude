# geodude 🪨

A geolocation microservice written in Rust. Geodude accepts an IP address and returns location information for it, using [IP2Location](https://www.ip2location.com/) as its underlying geolocation data source.

## Overview

Given an IP address (IPv4 or IPv6), geodude resolves it to location data such as country, region, city, and coordinates. It is designed to be deployed as a small, self-contained service that other applications can call when they need to geolocate a request.

## Data source

Geodude uses IP2Location databases for lookups. You will need to supply an IP2Location database file (e.g. `IP2LOCATION-LITE-DB*.BIN`) at runtime. See [ip2location.com](https://www.ip2location.com/) for available databases and licensing.

## Building

```sh
cargo build --release
```

## Running

```sh
cargo run --release
```

For local development, download a `.BIN` from IP2Location manually and point geodude at it via `IP2LOCATION_BIN_PATH`.

## Deploying to Railway

Geodude is published as a Docker image. On container startup, `entrypoint.sh` downloads the `.BIN` from IP2Location using your account token, so the database is never baked into the image or committed to the repo.

### 1. Get an IP2Location download token

Sign in at [ip2location.com](https://www.ip2location.com/), open your account's [download page](https://www.ip2location.com/file-download), and copy your download token. Note the file code of the database you want (e.g. `DB11LITEBIN` for the LITE DB11 BIN).

### 2. Set Railway variables

In the service's **Variables** tab, add:

| Variable                | Example         | Purpose                                                                 |
| ----------------------- | --------------- | ----------------------------------------------------------------------- |
| `IP2LOCATION_TOKEN`     | `xxxxxxxxxxxx`  | Your IP2Location download token. Keep this secret.                      |
| `IP2LOCATION_FILE_CODE` | `DB11LITEBIN`   | The file code of the database edition to download.                      |
| `IP2LOCATION_BIN_PATH`  | `/data/ip2location.BIN` | Optional. Where the BIN is written inside the container. Defaults to `/data/ip2location.BIN`. |
| `IP2LOCATION_MAX_AGE_DAYS` | `30`         | Optional. How old the BIN can get before the entrypoint re-downloads it on boot. Defaults to `30`. |

### 3. (Optional) Attach a volume

If you'd rather not re-download on every deploy, attach a Railway volume mounted at `/data` (or whichever directory contains `IP2LOCATION_BIN_PATH`). The entrypoint downloads when the file is missing **or** older than `IP2LOCATION_MAX_AGE_DAYS` (default 30), so a persisted volume short-circuits subsequent boots while still picking up monthly IP2Location updates automatically.

If the BIN is stale but the download fails (e.g. quota exceeded or token unset), the service keeps running with the existing file and logs a warning — it only hard-fails when there is no BIN at all.

Without a volume, the database is fetched fresh on every cold start — fine for an internal API and the simplest way to pick up monthly DB updates (just redeploy).

### 4. Deploy

Push to the branch Railway is tracking, or run `railway up`. Railway builds the Dockerfile, the container boots, the entrypoint pulls the BIN, and the service starts.

## Refreshing the database

IP2Location updates their databases monthly. The entrypoint refreshes automatically when the BIN is missing or older than `IP2LOCATION_MAX_AGE_DAYS` (default `30`), so on a normal cadence you don't need to do anything — the next restart after the file ages past the threshold pulls a new copy.

To force an immediate refresh:

- **Without a volume:** redeploy the service. The entrypoint downloads the current edition on boot.
- **With a volume:** `railway ssh` in, delete the existing `.BIN`, and restart the service — or temporarily detach the volume and redeploy. You can also lower `IP2LOCATION_MAX_AGE_DAYS` (e.g. to `0`) and restart to trigger the refresh path.
