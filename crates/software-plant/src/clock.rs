//! The clock simulator: the time each run of the binary under test reads, given through
//! `SOURCE_DATE_EPOCH`, and, where its time passes, whether time moves only as the run waits,
//! through `LOFT_CLOCK`.

use std::cell::Cell;
use std::process::Command;

/// The environment variable through which a run reads its time, in seconds since the Unix epoch.
pub const EPOCH_ENV: &str = "SOURCE_DATE_EPOCH";

/// The environment variable through which a run lets its time pass only as it waits.
pub const PASSING_ENV: &str = "LOFT_CLOCK";

/// The times of successive runs: fixed, stepping, or the system clock's; and whether, within a
/// run, time passes only as the run waits.
#[derive(Debug, Clone)]
pub struct Clock {
    next: Cell<Option<u64>>,
    step: u64,
    passing: bool,
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
            passing: false,
        }
    }

    /// No scenario: each run reads the system clock.
    #[must_use]
    pub fn system() -> Self {
        Self {
            next: Cell::new(None),
            step: 0,
            passing: false,
        }
    }

    /// The same scenario, in whose runs time passes only as each run waits: a wait returns at once
    /// and moves the run's time on by its length.
    #[must_use]
    pub fn passing(mut self) -> Self {
        self.passing = true;
        self
    }

    /// Whether time passes only as each run waits.
    #[must_use]
    pub fn is_passing(&self) -> bool {
        self.passing
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
        };
        if self.passing {
            command.env(PASSING_ENV, "passing")
        } else {
            command.env_remove(PASSING_ENV)
        }
    }
}
