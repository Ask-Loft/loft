//! The clock port: the time now, in whole seconds since the Unix epoch, from
//! `SOURCE_DATE_EPOCH` when it is set and from the system clock otherwise; and the waits a
//! command makes. Where `LOFT_CLOCK=passing` is set, as the Software Plant's clock simulator sets
//! it, a wait returns at once and the time moves on by its length instead.

use std::ffi::OsString;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::PortError;

/// The environment variable that names the time, for a reproducible run.
pub const EPOCH_ENV: &str = "SOURCE_DATE_EPOCH";

/// The environment variable that lets time pass only as the run waits, when it is `passing`.
pub const PASSING_ENV: &str = "LOFT_CLOCK";

/// The value of [`PASSING_ENV`] that lets time pass only as the run waits.
pub const PASSING: &str = "passing";

/// Where a command learns the time, and how it waits.
pub trait Clock {
    /// The time now, in whole seconds since the Unix epoch.
    ///
    /// # Errors
    ///
    /// The time cannot be read.
    fn now(&self) -> Result<u64, PortError>;

    /// How long has passed since the clock was made: a reading that never goes back, by which a
    /// command measures a wait.
    fn elapsed(&self) -> Duration;

    /// Waits for `length`.
    fn wait(&self, length: Duration);
}

/// The system clock, unless an epoch is given; time passes on its own, unless it passes only as
/// the clock waits.
#[derive(Debug, Clone)]
pub struct SystemClock {
    epoch: Option<OsString>,
    passing: bool,
    made: Instant,
    made_at: SystemTime,
    /// How long the clock has waited, where time passes only as it waits; shared by its clones,
    /// which are one clock.
    waited: Arc<Mutex<Duration>>,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::with_epoch(None::<OsString>)
    }
}

impl SystemClock {
    /// The clock as the process's environment sets it.
    #[must_use]
    pub fn from_env() -> Self {
        let passing = std::env::var_os(PASSING_ENV).is_some_and(|value| value == PASSING);
        Self::with_epoch(std::env::var_os(EPOCH_ENV)).passing_if(passing)
    }

    /// The clock with `SOURCE_DATE_EPOCH` set to `epoch`, or unset, whose time passes on its own.
    #[must_use]
    pub fn with_epoch(epoch: Option<impl Into<OsString>>) -> Self {
        Self {
            epoch: epoch.map(Into::into),
            passing: false,
            made: Instant::now(),
            made_at: SystemTime::now(),
            waited: Arc::new(Mutex::new(Duration::ZERO)),
        }
    }

    /// The same clock, whose time passes only as it waits where `passing` holds.
    #[must_use]
    pub fn passing_if(mut self, passing: bool) -> Self {
        self.passing = passing;
        self
    }

    fn waited(&self) -> Duration {
        *self
            .waited
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The system time the clock reads: when it was made, where time passes only as it waits.
    fn clock_time(&self) -> SystemTime {
        if self.passing {
            self.made_at
        } else {
            SystemTime::now()
        }
    }

    fn epoch(&self) -> Result<u64, PortError> {
        match &self.epoch {
            Some(epoch) => epoch
                .to_str()
                .filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
                .and_then(|text| text.parse().ok())
                .ok_or_else(|| PortError::Epoch(epoch.to_string_lossy().into_owned())),
            None => Ok(self
                .clock_time()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| PortError::Epoch("a system time before 1970".to_owned()))?
                .as_secs()),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Result<u64, PortError> {
        let epoch = self.epoch()?;
        Ok(if self.passing {
            epoch + self.waited().as_secs()
        } else {
            epoch
        })
    }

    fn elapsed(&self) -> Duration {
        if self.passing {
            self.waited()
        } else {
            self.made.elapsed()
        }
    }

    fn wait(&self, length: Duration) {
        if self.passing {
            let mut waited = self
                .waited
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *waited += length;
        } else {
            thread::sleep(length);
        }
    }
}
