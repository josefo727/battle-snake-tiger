//! Process settings: where the server listens and where it keeps its log.

use core::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;

const DEFAULT_BIND_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_LOG_KEEP_DAYS: usize = 14;

/// Where the server listens, read from `BIND_ADDR` (default `0.0.0.0`) and `PORT`
/// (default `8080`), and where it keeps its daily log files: `LOG_DIR` (unset or empty
/// means no files) and `LOG_KEEP_DAYS` (default 14).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub bind_addr: IpAddr,
    pub port: u16,
    pub log_dir: Option<PathBuf>,
    pub log_keep_days: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsError {
    InvalidBindAddr(String),
    InvalidPort(String),
    InvalidLogKeepDays(String),
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
            Self::InvalidLogKeepDays(value) => write!(
                formatter,
                "LOG_KEEP_DAYS {value:?} is not a number of days; use a whole number of 1 or more"
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
        let log_dir = lookup("LOG_DIR")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from);
        let log_keep_days = read(
            &lookup,
            "LOG_KEEP_DAYS",
            DEFAULT_LOG_KEEP_DAYS,
            SettingsError::InvalidLogKeepDays,
        )?;
        if log_keep_days == 0 {
            return Err(SettingsError::InvalidLogKeepDays("0".to_owned()));
        }
        Ok(Self {
            bind_addr,
            port,
            log_dir,
            log_keep_days,
        })
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
