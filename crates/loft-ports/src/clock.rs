//! The clock port: the time now, in whole seconds since the Unix epoch, from
//! `SOURCE_DATE_EPOCH` when it is set and from the system clock otherwise.

use std::ffi::OsString;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::PortError;

/// The environment variable that names the time, for a reproducible run.
pub const EPOCH_ENV: &str = "SOURCE_DATE_EPOCH";

/// Where a command learns the time.
pub trait Clock {
    /// The time now, in whole seconds since the Unix epoch.
    ///
    /// # Errors
    ///
    /// The time cannot be read.
    fn now(&self) -> Result<u64, PortError>;
}

/// The system clock, unless an epoch is given.
#[derive(Debug, Clone, Default)]
pub struct SystemClock {
    epoch: Option<OsString>,
}

impl SystemClock {
    /// The clock as the process's environment sets it.
    #[must_use]
    pub fn from_env() -> Self {
        Self::with_epoch(std::env::var_os(EPOCH_ENV))
    }

    /// The clock with `SOURCE_DATE_EPOCH` set to `epoch`, or unset.
    #[must_use]
    pub fn with_epoch(epoch: Option<impl Into<OsString>>) -> Self {
        Self {
            epoch: epoch.map(Into::into),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Result<u64, PortError> {
        match &self.epoch {
            Some(epoch) => epoch
                .to_str()
                .filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
                .and_then(|text| text.parse().ok())
                .ok_or_else(|| PortError::Epoch(epoch.to_string_lossy().into_owned())),
            None => Ok(SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| PortError::Epoch("a system time before 1970".to_owned()))?
                .as_secs()),
        }
    }
}
