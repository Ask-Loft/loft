//! The clock simulator: the time each run of the binary under test reads, given through
//! `SOURCE_DATE_EPOCH`.

use std::cell::Cell;
use std::process::Command;

/// The environment variable through which a run reads its time, in seconds since the Unix epoch.
pub const EPOCH_ENV: &str = "SOURCE_DATE_EPOCH";

/// The times of successive runs: fixed, stepping, or the system clock's.
#[derive(Debug, Clone)]
pub struct Clock {
    next: Cell<Option<u64>>,
    step: u64,
}

impl Clock {
    /// Every run at `epoch`.
    #[must_use]
    pub fn fixed(epoch: u64) -> Self {
        Self::stepping(epoch, 0)
    }

    /// The first run at `start`, and each later run `step` seconds after the one before.
    #[must_use]
    pub fn stepping(start: u64, step: u64) -> Self {
        Self {
            next: Cell::new(Some(start)),
            step,
        }
    }

    /// No scenario: each run reads the system clock.
    #[must_use]
    pub fn system() -> Self {
        Self {
            next: Cell::new(None),
            step: 0,
        }
    }

    /// The value of [`EPOCH_ENV`] for the next run, or none, and advances to the run after it.
    #[must_use]
    pub fn next_epoch(&self) -> Option<String> {
        let now = self.next.get()?;
        self.next.set(Some(now + self.step));
        Some(now.to_string())
    }

    /// Gives `command` the time of the next run.
    pub fn apply<'a>(&self, command: &'a mut Command) -> &'a mut Command {
        match self.next_epoch() {
            Some(epoch) => command.env(EPOCH_ENV, epoch),
            None => command.env_remove(EPOCH_ENV),
        }
    }
}
