//! How long a flush keeps away from its address after runs that made a request and failed: 5
//! seconds after the first such run in a row, doubled after each further one up to 5 minutes, and
//! drawn at random between half of it and all of it, as the clients of AWS and gRPC back off, so
//! that many machines that failed together do not come back together. A batch that goes clears
//! the count, and a `Retry-After` takes the pause's place and leaves the count as it was.

use super::sending::Flushed;

/// The pause after the first failed run in a row, in seconds.
const FIRST: u64 = 5;

/// The longest pause, in seconds.
const MOST: u64 = 5 * 60;

/// The pause after `failures` runs in a row that failed, in whole seconds from half of it, rounded
/// up, to all of it, picked by `draw`.
#[must_use]
pub(super) fn pause(failures: u32, draw: u64) -> u64 {
    // Past 16 doublings the pause has long reached its most.
    let full = (FIRST << failures.saturating_sub(1).min(16)).min(MOST);
    let least = full.div_ceil(2);
    least + draw % (full - least + 1)
}

/// A number drawn at random, or 0 where the system gives no random bytes.
#[must_use]
pub(super) fn draw() -> u64 {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).map_or(0, |()| u64::from_le_bytes(bytes))
}

/// The failed runs in a row once `flushed`, a run that sent, has ended at `ended`, after
/// `previous` of them, and the time before which no run makes a request, where the run sets one:
/// its `Retry-After`, or a pause picked by `draw`.
#[must_use]
pub(super) fn after(previous: u32, flushed: &Flushed, ended: u64, draw: u64) -> (u32, Option<u64>) {
    let base = if flushed.went { 0 } else { previous };
    if flushed.retry_after.is_some() {
        return (base, flushed.retry_after);
    }
    let failed = flushed
        .failed
        .iter()
        .any(|failed| failed.kind == "yard-server");
    if failed {
        let failures = base.saturating_add(1);
        (failures, Some(ended.saturating_add(pause(failures, draw))))
    } else {
        (base, None)
    }
}

#[cfg(test)]
mod tests {
    use super::super::settle::Failed;
    use super::*;

    #[test]
    fn the_first_pause_runs_from_2_5_to_5_seconds_rounded_up() {
        assert_eq!((pause(1, 0), pause(1, 2), pause(1, 3)), (3, 5, 3));
    }

    #[test]
    fn each_further_failure_doubles_the_pause_up_to_5_minutes() {
        assert_eq!((pause(2, 0), pause(2, 5)), (5, 10));
        assert_eq!((pause(3, 0), pause(3, 10)), (10, 20));
        assert_eq!((pause(7, 0), pause(7, 150)), (150, 300));
        assert_eq!((pause(8, 0), pause(8, 150)), (150, 300));
        assert_eq!((pause(u32::MAX, 0), pause(u32::MAX, 150)), (150, 300));
        assert!((150..=300).contains(&pause(9, u64::MAX)));
    }

    fn run(went: bool, failed: Vec<Failed>) -> Flushed {
        Flushed {
            failed,
            went,
            ..Flushed::quiet("failed")
        }
    }

    #[test]
    fn a_run_that_made_a_request_and_failed_adds_a_failure_and_a_pause() {
        let failed = run(false, vec![Failed::yard_server("500".to_owned())]);
        assert_eq!(after(2, &failed, 1000, 10), (3, Some(1020)));
    }

    #[test]
    fn a_batch_that_goes_clears_the_failures_before_a_later_one_fails() {
        assert_eq!(after(5, &run(true, Vec::new()), 1000, 0), (0, None));
        let then_failed = run(true, vec![Failed::yard_server("500".to_owned())]);
        assert_eq!(after(5, &then_failed, 1000, 2), (1, Some(1005)));
        let then_paused = Flushed {
            went: true,
            retry_after: Some(1120),
            ..Flushed::quiet("paused")
        };
        assert_eq!(after(5, &then_paused, 1000, 0), (0, Some(1120)));
    }

    #[test]
    fn a_retry_after_or_a_failure_of_the_outbox_leaves_the_failures_as_they_were() {
        let paused = Flushed {
            retry_after: Some(1120),
            ..Flushed::quiet("paused")
        };
        assert_eq!(after(2, &paused, 1000, 0), (2, Some(1120)));
        let outbox = run(false, vec![Failed::outbox("cannot drop".to_owned())]);
        assert_eq!(after(2, &outbox, 1000, 0), (2, None));
    }
}
