//! HTTP client for a [geodude](https://github.com/marchsabino/geodude) server.
//!
//! # Quickstart
//!
//! Point at your geodude server once, then call `locate` from anywhere:
//!
//! ```no_run
//! # async fn run() -> Result<(), geodude::Error> {
//! geodude::setup("https://geodude.example.com")?;
//! let loc = geodude::locate("8.8.8.8").await?;
//! println!("{:?}", loc.country_name);
//! # Ok(()) }
//! ```
//!
//! Or read the URL from the `GEODUDE_URL` environment variable:
//!
//! ```no_run
//! # async fn run() -> Result<(), geodude::Error> {
//! geodude::from_env()?;
//! let loc = geodude::locate("8.8.8.8").await?;
//! # Ok(()) }
//! ```
//!
//! `locate` lazily falls back to `GEODUDE_URL` on first call if neither
//! [`setup`] nor [`from_env`] has run, so the env-only path can skip the
//! init call entirely.

use std::net::{AddrParseError, IpAddr, Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};

/// Geolocation data resolved from an IP address. Matches the JSON the geodude
/// server returns from `GET /geolocation/{ip}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

/// Errors returned by the geodude client.
#[derive(Debug)]
pub enum Error {
    InvalidAddress(AddrParseError),
    NotFound,
    NotConfigured,
    InvalidUrl(String),
    #[cfg(feature = "client")]
    Http(reqwest::Error),
    Status {
        status: u16,
        message: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAddress(e) => write!(f, "invalid IP address: {e}"),
            Self::NotFound => write!(f, "no record found"),
            Self::NotConfigured => write!(
                f,
                "geodude is not configured; call geodude::setup() or set GEODUDE_URL"
            ),
            Self::InvalidUrl(s) => write!(f, "invalid geodude URL: {s}"),
            #[cfg(feature = "client")]
            Self::Http(e) => write!(f, "http error: {e}"),
            Self::Status { status, message } => {
                if message.is_empty() {
                    write!(f, "geodude server returned {status}")
                } else {
                    write!(f, "geodude server returned {status}: {message}")
                }
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidAddress(e) => Some(e),
            #[cfg(feature = "client")]
            Self::Http(e) => Some(e),
            _ => None,
        }
    }
}

impl From<AddrParseError> for Error {
    fn from(e: AddrParseError) -> Self {
        Self::InvalidAddress(e)
    }
}

/// Anything that can be turned into an [`IpAddr`].
///
/// Implemented for `IpAddr`, `Ipv4Addr`, `Ipv6Addr`, `&str`, `&String`, `String`.
pub trait IntoIpAddr {
    fn into_ip_addr(self) -> Result<IpAddr, AddrParseError>;
}

impl IntoIpAddr for IpAddr {
    fn into_ip_addr(self) -> Result<IpAddr, AddrParseError> {
        Ok(self)
    }
}

impl IntoIpAddr for Ipv4Addr {
    fn into_ip_addr(self) -> Result<IpAddr, AddrParseError> {
        Ok(IpAddr::V4(self))
    }
}

impl IntoIpAddr for Ipv6Addr {
    fn into_ip_addr(self) -> Result<IpAddr, AddrParseError> {
        Ok(IpAddr::V6(self))
    }
}

impl IntoIpAddr for &str {
    fn into_ip_addr(self) -> Result<IpAddr, AddrParseError> {
        self.parse()
    }
}

impl IntoIpAddr for &String {
    fn into_ip_addr(self) -> Result<IpAddr, AddrParseError> {
        self.parse()
    }
}

impl IntoIpAddr for String {
    fn into_ip_addr(self) -> Result<IpAddr, AddrParseError> {
        self.parse()
    }
}

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
pub use client::{Client, configured_url, from_env, locate, setup};
