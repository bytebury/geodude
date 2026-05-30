# geodude 🪨

IP geolocation in Rust, backed by [IP2Location](https://www.ip2location.com/). Geodude ships as:

- an **HTTP microservice** that owns the IP2Location database, and
- a **Rust client crate** that other services use to query it.

The pattern is: deploy one geodude server somewhere, then `cargo add geodude` in every other Rust service that needs geolocation. The server keeps the BIN and the IP2Location credentials in one place; clients just make HTTP calls.

## Client usage

```toml
[dependencies]
geodude = "0.1"  # client by default; no need to disable anything
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Point at your server once and call from anywhere:

```rust
geodude::setup("https://geodude.example.com")?;
let loc = geodude::locate("8.8.8.8").await?;
println!("{:?}", loc.country_name);
```

Or let it pick up the URL from the environment:

```rust
let loc = geodude::locate("8.8.8.8").await?; // reads GEODUDE_URL from environment variables
```

`locate` lazily falls back to `GEODUDE_URL` on first call, so for the env-only path you can skip `from_env()` entirely. For multiple servers or finer control over the `reqwest::Client`, use `geodude::Client::new(url)` directly. `locate` accepts `IpAddr`, `Ipv4Addr`, `Ipv6Addr`, `&str`, `&String`, or `String` via the `IntoIpAddr` trait.

### Cargo features

| Feature  | Default | Purpose                                                                                           |
| -------- | ------- | ------------------------------------------------------------------------------------------------- |
| `client` | yes     | Builds the HTTP client (`reqwest` + `serde_json`).                                                |
| `server` | no      | Builds the geodude server binary; pulls in axum, tokio, tracing, dotenvy, and `ip2location`.       |

Run the server locally with `cargo run --no-default-features --features server`. The Dockerfile and `dev.sh` already pass these flags.

## Server

## Data source

Geodude uses IP2Location databases for lookups. You will need to supply an IP2Location database file (e.g. `IP2LOCATION-LITE-DB*.BIN`) at runtime. See [ip2location.com](https://www.ip2location.com/) for available databases and licensing.

## Development mode (microservice)

```sh
./dev.sh
```

## Building

```sh
cargo build --release
```

## Running

```sh
cargo run --release
```

For local development, download a `.BIN` from IP2Location manually and point geodude at it via `IP2LOCATION_BIN_PATH`.

## Deploying 

Geodude is published as a Docker image. On container startup, `entrypoint.sh` downloads the `.BIN` from IP2Location using your account token, so the database is never baked into the image or committed to the repo.

### 1. Get an IP2Location download token

Sign in at [ip2location.com](https://www.ip2location.com/), open your account's [download page](https://www.ip2location.com/file-download), and copy your download token. Note the file code of the database you want (e.g. `DB11LITEBIN` for the LITE DB11 BIN).

### 2. Set Environment variables

In the service's **Variables** tab, add:

| Variable                | Example         | Purpose                                                                 |
| ----------------------- | --------------- | ----------------------------------------------------------------------- |
| `IP2LOCATION_TOKEN`     | `xxxxxxxxxxxx`  | Your IP2Location download token. Keep this secret.                      |
| `IP2LOCATION_FILE_CODE` | `DB11LITEBIN`   | The file code of the database edition to download.                      |
| `IP2LOCATION_BIN_PATH`  | `/data/ip2location.BIN` | Optional. Where the BIN is written inside the container. Defaults to `/data/ip2location.BIN`. |
| `IP2LOCATION_MAX_AGE_DAYS` | `30`         | Optional. How old the BIN can get before the entrypoint re-downloads it on boot. Defaults to `30`. |
| `ADMIN_REFRESH_TOKEN`   | `xxxxxxxxxxxx`  | Optional. Shared secret required to call `POST /admin/refresh-db`. Leave unset to disable the endpoint (returns `503`). |

### 3. (Optional) Attach a volume

If you'd rather not re-download on every deploy, attach a volume mounted at `/data` (or whichever directory contains `IP2LOCATION_BIN_PATH`). The entrypoint downloads when the file is missing **or** older than `IP2LOCATION_MAX_AGE_DAYS` (default 30), so a persisted volume short-circuits subsequent boots while still picking up monthly IP2Location updates automatically.

If the BIN is stale but the download fails (e.g. quota exceeded or token unset), the service keeps running with the existing file and logs a warning — it only hard-fails when there is no BIN at all.

Without a volume, the database is fetched fresh on every cold start — fine for an internal API and the simplest way to pick up monthly DB updates (just redeploy).

## Refreshing the database

IP2Location updates their databases monthly. The entrypoint refreshes automatically when the BIN is missing or older than `IP2LOCATION_MAX_AGE_DAYS` (default `30`) on boot. For long-running deploys with a persistent volume, a cron job can hit the refresh endpoint instead, avoiding a restart.

### Scheduled refresh via `/admin/refresh-db`

Set `ADMIN_REFRESH_TOKEN` and `POST /admin/refresh-db` once a month with the matching bearer token. The handler:

1. Returns `401` unless `Authorization: Bearer $ADMIN_REFRESH_TOKEN` matches.
2. Stats the BIN. If it is younger than `IP2LOCATION_MAX_AGE_DAYS`, returns `200 {"status":"skipped",...}` without contacting IP2Location.
3. Otherwise runs `refresh-db.sh` (which downloads, unzips, and atomically replaces the BIN), reloads the in-memory database, and returns `200 {"status":"refreshed",...}`.

Example cron entry (runs daily at 03:00 UTC; the handler itself short-circuits until the BIN ages past the threshold):

```cron
0 3 * * * curl -fsS -X POST -H "Authorization: Bearer $ADMIN_REFRESH_TOKEN" https://geodude.example.com/admin/refresh-db
```
