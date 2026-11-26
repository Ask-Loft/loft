//! The sending (the Send versions to Yard Server Feature): one run takes the oldest entries of
//! the outbox for its address, at most 16 and at most 1 MiB of them, sends them as one batch of
//! versions through the network client, settles each entry by Yard Server's answer, and takes the
//! next batch, until no entry is left that it has not taken in this run, a batch does not go, or
//! 5 minutes have passed. Entries put into the outbox while a batch is on its way go in a later
//! batch of the same run. Where Yard Server asks it to come back later, the run ends `paused`.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use loft_ports::clock::Clock;
use yardlog::batch::{MOST_BYTES, MOST_RECORDS};
use yardlog_store::outbox::Outbox;

use super::settle::{self, Failed};

/// How long a run takes new batches: after it, the run ends once the batch on its way is settled.
pub(super) const MOST_TIME: Duration = Duration::from_mins(5);

/// What one run did.
#[derive(Debug)]
pub(super) struct Flushed {
    pub outcome: &'static str,
    /// The versions Yard Server stored or already held, in every batch of the run.
    pub sent: usize,
    /// The entries for the address left in the outbox, other than those put off and skipped.
    pub remaining: usize,
    /// The entries whose versions Yard Server will take later.
    pub later: usize,
    /// The entries of a newer format passed over.
    pub skipped: usize,
    /// The files moved into `dead/` in the run.
    pub dead: usize,
    /// Why a batch did not go, or an entry could not be settled, where one did not or could not.
    pub failed: Vec<Failed>,
    /// The keys of the entries the run took, whatever became of them.
    pub taken: BTreeSet<String>,
    /// The versions Yard Server will take later, by key, each with its code.
    pub put_off: BTreeMap<String, String>,
    /// The keys of the entries for the address left in the outbox, put off ones included.
    pub waiting: BTreeSet<String>,
    /// The time Yard Server asked the run not to come back before, in seconds.
    pub not_before: Option<u64>,
}

impl Flushed {
    /// The result of a run that did not look at the outbox.
    pub fn quiet(outcome: &'static str) -> Self {
        Self {
            outcome,
            sent: 0,
            remaining: 0,
            later: 0,
            skipped: 0,
            dead: 0,
            failed: Vec::new(),
            taken: BTreeSet::new(),
            put_off: BTreeMap::new(),
            waiting: BTreeSet::new(),
            not_before: None,
        }
    }

    pub fn failed(failed: Failed) -> Self {
        Self {
            failed: vec![failed],
            ..Self::quiet("failed")
        }
    }
}

/// Runs the sending of `outbox` with `credential`, its token, waiting at most `wait` for each
/// answer and reading the time from `clock`.
pub(super) fn drain(
    outbox: &Outbox,
    credential: &str,
    wait: Duration,
    clock: &dyn Clock,
) -> Flushed {
    let began = clock.elapsed();
    let mut flushed = Flushed::quiet("empty");
    while flushed.failed.is_empty()
        && flushed.not_before.is_none()
        && clock.elapsed().saturating_sub(began) < MOST_TIME
    {
        let taken = match outbox.take_after(MOST_RECORDS, MOST_BYTES, &flushed.taken) {
            Ok(taken) => taken,
            Err(error) => {
                flushed.failed.push(Failed::outbox(error.to_string()));
                break;
            }
        };
        flushed.dead += taken.dead;
        flushed.skipped = taken.skipped;
        if taken.entries.is_empty() {
            break;
        }
        flushed.outcome = "sent";
        let keys = taken.entries.iter().map(|entry| entry.key.clone());
        flushed.taken.extend(keys);
        let now = clock.now().unwrap_or_default();
        let settled = settle::send(outbox, credential, &taken.entries, wait, now);
        flushed.sent += settled.sent;
        flushed.dead += settled.dead;
        flushed.put_off.extend(settled.later);
        flushed.failed.extend(settled.failed);
        flushed.not_before = settled.come_back.map(|seconds| now.saturating_add(seconds));
    }
    if !flushed.failed.is_empty() {
        flushed.outcome = "failed";
    } else if flushed.not_before.is_some() {
        flushed.outcome = "paused";
    }
    flushed.later = flushed.put_off.len();
    flushed.waiting = super::waiting(outbox);
    flushed.remaining = flushed
        .waiting
        .iter()
        .filter(|key| !flushed.put_off.contains_key(*key))
        .count();
    flushed
}
