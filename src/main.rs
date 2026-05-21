use std::env;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::get,
};
use ip2location::{DB, Record, error::Error as Ip2LocationError};
use serde::Serialize;

struct AppState {
    db: DB,
}

#[derive(Debug, Serialize)]
struct GeoResponse {
    ip: String,
    country_code: Option<String>,
    country_name: Option<String>,
    region: Option<String>,
    city: Option<String>,
    latitude: Option<f32>,
    longitude: Option<f32>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
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

    let bin_path = env::var("IP2LOCATION_BIN_PATH")
        .unwrap_or_else(|_| "/data/ip2location.BIN".to_string());
    let bin_size = std::fs::metadata(&bin_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let db = DB::from_file(&bin_path)
        .map_err(|e| format!("failed to open {bin_path}: {e:?}"))?;
    tracing::info!("loaded IP2Location database from {bin_path} ({bin_size} bytes)");
    db.print_db_info();

    let state = Arc::new(AppState { db });

    let app = Router::new()
        .route("/geolocation/{ip_address}", get(geo_lookup))
        .route("/health", get(health))
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

async fn geo_lookup(
    State(state): State<Arc<AppState>>,
    Path(ip): Path<String>,
) -> Result<Json<GeoResponse>, (StatusCode, Json<ErrorResponse>)> {
    let parsed: IpAddr = ip.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("invalid IP address: {ip}"),
            }),
        )
    })?;

    let record = state.db.ip_lookup(parsed).map_err(|e| match e {
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
        Record::LocationDb(loc) => GeoResponse {
            ip,
            country_code: loc.country.as_ref().map(|c| c.short_name.to_string()),
            country_name: loc.country.as_ref().map(|c| c.long_name.to_string()),
            region: loc.region.as_ref().map(|s| s.to_string()),
            city: loc.city.as_ref().map(|s| s.to_string()),
            latitude: loc.latitude,
            longitude: loc.longitude,
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
