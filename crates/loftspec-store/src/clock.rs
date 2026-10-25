//! The clock port: the time of a publish, in whole seconds since the Unix epoch, from
//! `SOURCE_DATE_EPOCH` when it is set and from the system clock otherwise. The clock itself is
//! `loft-ports'`; this module gives it the store's errors.

use std::ffi::OsString;

use loft_ports::clock::Clock as _;

use crate::StoreError;

/// The environment variable that names a publish time, for a reproducible publish.
pub const EPOCH_ENV: &str = loft_ports::clock::EPOCH_ENV;

/// Where a publish learns the time.
pub trait Clock {
    /// The time now, in whole seconds since the Unix epoch.
    ///
    /// # Errors
    ///
    /// The time cannot be read.
    fn now(&self) -> Result<u64, StoreError>;
}

/// The system clock, unless an epoch is given.
#[derive(Debug, Clone, Default)]
pub struct SystemClock(loft_ports::clock::SystemClock);

impl SystemClock {
    /// The clock as the process's environment sets it.
    #[must_use]
    pub fn from_env() -> Self {
        Self(loft_ports::clock::SystemClock::from_env())
    }

    /// The clock with `SOURCE_DATE_EPOCH` set to `epoch`, or unset.
    #[must_use]
    pub fn with_epoch(epoch: Option<impl Into<OsString>>) -> Self {
        Self(loft_ports::clock::SystemClock::with_epoch(epoch))
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Result<u64, StoreError> {
        Ok(self.0.now()?)
    }
}
