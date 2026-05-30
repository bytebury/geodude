use std::env;
use std::sync::OnceLock;

use reqwest::{IntoUrl, StatusCode, Url};

use crate::{Error, IntoIpAddr, Location};

/// HTTP client for a geodude server.
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: Url,
}

impl Client {
    /// Construct a client pointed at `base_url` (e.g. `https://geodude.example.com`).
    pub fn new<U: IntoUrl>(base_url: U) -> Result<Self, Error> {
        let base_url = base_url
            .into_url()
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
        })
    }

    /// Construct a client using a caller-supplied `reqwest::Client` (so you can
    /// share connection pools, set timeouts, install middleware, etc.).
    pub fn with_http<U: IntoUrl>(http: reqwest::Client, base_url: U) -> Result<Self, Error> {
        let base_url = base_url
            .into_url()
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        Ok(Self { http, base_url })
    }

    /// Base URL the client points at.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Look up the location of `ip` by calling `GET {base}/geolocation/{ip}`.
    pub async fn locate(&self, ip: impl IntoIpAddr) -> Result<Location, Error> {
        let ip = ip.into_ip_addr()?;
        let url = format!(
            "{}/geolocation/{ip}",
            self.base_url.as_str().trim_end_matches('/')
        );
        let resp = self.http.get(url).send().await.map_err(Error::Http)?;
        let status = resp.status();
        if status.is_success() {
            resp.json::<Location>().await.map_err(Error::Http)
        } else if status == StatusCode::NOT_FOUND {
            Err(Error::NotFound)
        } else {
            let message = resp.text().await.unwrap_or_default();
            Err(Error::Status {
                status: status.as_u16(),
                message: extract_error_message(&message),
            })
        }
    }
}

/// The geodude server's error responses look like `{"error":"..."}`. Pull the
/// inner string out when present; otherwise return the body verbatim.
fn extract_error_message(body: &str) -> String {
    #[derive(serde::Deserialize)]
    struct ErrorBody {
        error: String,
    }
    serde_json::from_str::<ErrorBody>(body)
        .map(|b| b.error)
        .unwrap_or_else(|_| body.to_string())
}

static GLOBAL: OnceLock<Client> = OnceLock::new();

/// Configure the process-global client with an explicit URL. The first
/// successful call wins; subsequent calls are no-ops.
pub fn setup<U: IntoUrl>(base_url: U) -> Result<(), Error> {
    let client = Client::new(base_url)?;
    let _ = GLOBAL.set(client);
    Ok(())
}

/// Configure the process-global client by reading `GEODUDE_URL`.
pub fn from_env() -> Result<(), Error> {
    let url = env::var("GEODUDE_URL").map_err(|_| Error::NotConfigured)?;
    setup(url)
}

/// The configured base URL, if [`setup`] or [`from_env`] has run.
pub fn configured_url() -> Option<&'static Url> {
    GLOBAL.get().map(|c| c.base_url())
}

/// Look up `ip` via the process-global client. Lazily initializes from
/// `GEODUDE_URL` on first call if not already configured.
pub async fn locate(ip: impl IntoIpAddr) -> Result<Location, Error> {
    let client = match GLOBAL.get() {
        Some(c) => c,
        None => {
            from_env()?;
            GLOBAL.get().expect("from_env set global client")
        }
    };
    client.locate(ip).await
}
