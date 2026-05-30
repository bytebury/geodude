use std::env;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use axum::{
    Router,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::{Json, Response},
    routing::{get, post},
};
use geodude::Location;
use ip2location::{DB, Record, error::Error as Ip2LocationError};
use serde::Serialize;

struct AppState {
    db: RwLock<DB>,
    bin_path: PathBuf,
    max_age_days: u64,
    refresh_token: Option<String>,
    refresh_script: PathBuf,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct RefreshResponse {
    status: &'static str,
    age_days: u64,
    max_age_days: u64,
    size_bytes: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let bin_path = PathBuf::from(
        env::var("IP2LOCATION_BIN_PATH").unwrap_or_else(|_| "/data/ip2location.BIN".to_string()),
    );
    let max_age_days: u64 = env::var("IP2LOCATION_MAX_AGE_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let refresh_token = env::var("ADMIN_REFRESH_TOKEN")
        .ok()
        .filter(|s| !s.is_empty());
    let refresh_script = PathBuf::from(
        env::var("IP2LOCATION_REFRESH_SCRIPT")
            .unwrap_or_else(|_| "/app/refresh-db.sh".to_string()),
    );

    let bin_size = std::fs::metadata(&bin_path).map(|m| m.len()).unwrap_or(0);
    let db = DB::from_file(&bin_path)
        .map_err(|e| format!("failed to open {}: {e:?}", bin_path.display()))?;
    tracing::info!(
        "loaded IP2Location database from {} ({bin_size} bytes)",
        bin_path.display()
    );
    db.print_db_info();

    if refresh_token.is_none() {
        tracing::warn!(
            "ADMIN_REFRESH_TOKEN unset — /admin/refresh-db will return 503 until configured"
        );
    }

    let state = Arc::new(AppState {
        db: RwLock::new(db),
        bin_path,
        max_age_days,
        refresh_token,
        refresh_script,
    });

    let app = Router::new()
        .route("/geolocation/{ip_address}", get(geo_lookup))
        .route("/admin/refresh-db", post(refresh_db))
        .route("/health", get(health))
        .layer(middleware::from_fn(log_requests))
        .with_state(state);

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from((Ipv6Addr::UNSPECIFIED, port));
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn log_requests(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let response = next.run(req).await;
    tracing::info!("{method} {uri} -> {}", response.status());
    response
}

async fn geo_lookup(
    State(state): State<Arc<AppState>>,
    Path(ip): Path<String>,
) -> Result<Json<Location>, (StatusCode, Json<ErrorResponse>)> {
    let parsed: IpAddr = ip.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("invalid IP address: {ip}"),
            }),
        )
    })?;

    let db = state.db.read().expect("database lock poisoned");
    let record = db.ip_lookup(parsed).map_err(|e| match e {
        Ip2LocationError::RecordNotFound => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("no record found for {ip}"),
            }),
        ),
        other => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("lookup failed: {other:?}"),
            }),
        ),
    })?;

    let response = match record {
        Record::LocationDb(loc) => Location {
            country_code: loc.country.as_ref().map(|c| c.short_name.to_string()),
            country_name: loc.country.as_ref().map(|c| c.long_name.to_string()),
            region: loc.region.as_ref().map(|s| s.to_string()),
            city: loc.city.as_ref().map(|s| s.to_string()),
        },
        Record::ProxyDb(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "proxy databases are not supported by this endpoint".to_string(),
                }),
            ));
        }
    };

    tracing::info!("{response:?}");

    Ok(Json(response))
}

async fn refresh_db(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<RefreshResponse>), (StatusCode, Json<ErrorResponse>)> {
    let expected = state.refresh_token.as_deref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "refresh endpoint is not configured (ADMIN_REFRESH_TOKEN unset)"
                    .to_string(),
            }),
        )
    })?;

    let provided = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    if provided != Some(expected) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "invalid or missing bearer token".to_string(),
            }),
        ));
    }

    let metadata = std::fs::metadata(&state.bin_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to stat {}: {e}", state.bin_path.display()),
            }),
        )
    })?;
    let modified = metadata.modified().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to read mtime: {e}"),
            }),
        )
    })?;
    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::ZERO);
    let age_days = age.as_secs() / 86_400;
    let max_age = Duration::from_secs(state.max_age_days.saturating_mul(86_400));

    if age < max_age {
        tracing::info!(
            "refresh requested but BIN is {age_days}d old (threshold {}d); skipping",
            state.max_age_days
        );
        return Ok((
            StatusCode::OK,
            Json(RefreshResponse {
                status: "skipped",
                age_days,
                max_age_days: state.max_age_days,
                size_bytes: metadata.len(),
            }),
        ));
    }

    tracing::info!(
        "BIN is {age_days}d old (threshold {}d); running {}",
        state.max_age_days,
        state.refresh_script.display()
    );

    let output = tokio::process::Command::new(&state.refresh_script)
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to invoke {}: {e}", state.refresh_script.display()),
                }),
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.trim().is_empty() {
        tracing::info!("refresh-db.sh stdout:\n{}", stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        tracing::warn!("refresh-db.sh stderr:\n{}", stderr.trim_end());
    }

    if !output.status.success() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("refresh script exited {}", output.status),
            }),
        ));
    }

    let new_db = DB::from_file(&state.bin_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to reload {}: {e:?}", state.bin_path.display()),
            }),
        )
    })?;
    let new_size = std::fs::metadata(&state.bin_path)
        .map(|m| m.len())
        .unwrap_or(0);

    *state.db.write().expect("database lock poisoned") = new_db;

    tracing::info!("refreshed IP2Location database in place ({new_size} bytes)");

    Ok((
        StatusCode::OK,
        Json(RefreshResponse {
            status: "refreshed",
            age_days: 0,
            max_age_days: state.max_age_days,
            size_bytes: new_size,
        }),
    ))
}
