//! Process settings: where the server listens.

use core::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;

const DEFAULT_BIND_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
const DEFAULT_PORT: u16 = 8080;

/// Where the server listens, read from `BIND_ADDR` (default `0.0.0.0`) and `PORT`
/// (default `8080`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub bind_addr: IpAddr,
    pub port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsError {
    InvalidBindAddr(String),
    InvalidPort(String),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBindAddr(value) => write!(
                formatter,
                "BIND_ADDR {value:?} is not an IP address; use a form such as 0.0.0.0, 127.0.0.1 or ::1"
            ),
            Self::InvalidPort(value) => write!(
                formatter,
                "PORT {value:?} is not a port number; use a whole number from 0 to 65535"
            ),
        }
    }
}

impl std::error::Error for SettingsError {}

impl Settings {
    /// Reads the settings through `lookup` (variable name to value), so tests
    /// never touch the process environment.
    ///
    /// # Errors
    ///
    /// Fails when a variable is present but is not a valid value.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, SettingsError> {
        let bind_addr = read(
            &lookup,
            "BIND_ADDR",
            DEFAULT_BIND_ADDR,
            SettingsError::InvalidBindAddr,
        )?;
        let port = read(&lookup, "PORT", DEFAULT_PORT, SettingsError::InvalidPort)?;
        Ok(Self { bind_addr, port })
    }

    /// Reads the settings from the real process environment.
    ///
    /// # Errors
    ///
    /// Fails when a variable is present but is not a valid value.
    pub fn from_env() -> Result<Self, SettingsError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    #[must_use]
    pub const fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.bind_addr, self.port)
    }
}

/// The value of `name`, or `default` when it is not set; a value that is set
/// but does not parse becomes `invalid(raw text)`.
fn read<T: FromStr>(
    lookup: &impl Fn(&str) -> Option<String>,
    name: &str,
    default: T,
    invalid: fn(String) -> SettingsError,
) -> Result<T, SettingsError> {
    lookup(name).map_or(Ok(default), |raw| raw.parse().map_err(|_| invalid(raw)))
}
