//! The Codex Yard worker's flush (the Flush in the background Feature): at each beat where a
//! flush is due, the worker starts `loft yard flush` as a child, without waiting for it, so that
//! its heartbeat and its delivery of events go on. It starts no flush while one it started still
//! runs, stops one that still runs 6 minutes after it started, and stops one that runs when the
//! worker ends with its session.

use std::process::Child;
use std::time::Duration;

use loft_ports::clock::Clock;

use super::super::Result;

/// How long a flush the worker started may run before the worker stops it: a minute past the 5
/// minutes after which a flush ends by itself.
pub(super) const MOST_TIME: Duration = Duration::from_mins(6);

/// The flush the worker started, with the time on its clock it started at.
#[derive(Default)]
pub(super) struct Flushing(Option<(Child, Duration)>);

impl Drop for Flushing {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Flushing {
    /// On each tick: forgets a flush that has ended and stops one that has run too long; on a
    /// beat, and where `due` says a flush has work and none runs, starts one with `start`. A
    /// flush that cannot be started or watched is given up until the next due beat, so that the
    /// worker's heartbeat and its delivery of events go on whatever becomes of it.
    pub fn tick(
        &mut self,
        clock: &dyn Clock,
        beat: bool,
        due: impl FnOnce() -> bool,
        start: impl FnOnce() -> Result<Child>,
    ) {
        if let Some((child, started)) = &mut self.0 {
            match child.try_wait() {
                Ok(Some(_)) => self.0 = None,
                Ok(None) if clock.elapsed().saturating_sub(*started) < MOST_TIME => {}
                Ok(None) | Err(_) => self.stop(),
            }
        }
        if beat && self.0.is_none() && due() {
            self.0 = start().ok().map(|child| (child, clock.elapsed()));
        }
    }

    /// Whether a flush the worker started still runs.
    #[cfg(test)]
    pub fn runs(&self) -> bool {
        self.0.is_some()
    }

    /// Waits until the flush the worker started ends by itself, without stopping it.
    #[cfg(all(test, unix))]
    pub fn wait(&mut self) {
        if let Some((child, _)) = &mut self.0 {
            let _ = child.wait();
        }
    }

    fn stop(&mut self) {
        if let Some((mut child, _)) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
