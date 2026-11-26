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
    /// The keys `~/.yard/flush.json` records as those of versions Yard Server will take later.
    pub held: BTreeSet<String>,
    /// The time Yard Server asked the run not to come back before, in seconds.
    pub not_before: Option<u64>,
}

impl Flushed {
    /// The result of a run that did not look at the outbox.
    pub fn quiet(outcome: &'static str) -> Self {
        Self {
            outcome,
            sent: 0,
            skipped: 0,
            dead: 0,
            failed: Vec::new(),
            taken: BTreeSet::new(),
            put_off: BTreeMap::new(),
            waiting: BTreeSet::new(),
            held: BTreeSet::new(),
            not_before: None,
        }
    }

    pub fn failed(failed: Failed) -> Self {
        Self {
            failed: vec![failed],
            ..Self::quiet("failed")
        }
    }

    /// The entries for the address left in the outbox whose versions Yard Server will take later,
    /// as it answered in this run or as `~/.yard/flush.json` records.
    pub fn later(&self) -> usize {
        self.waiting.iter().filter(|key| self.is_later(key)).count()
    }

    /// The entries for the address left in the outbox, other than those put off and skipped.
    pub fn remaining(&self) -> usize {
        self.waiting
            .iter()
            .filter(|key| !self.is_later(key))
            .count()
    }

    fn is_later(&self, key: &str) -> bool {
        self.put_off.contains_key(key) || self.held.contains(key)
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
        if taken.entries.is_empty() {
            break;
        }
        flushed.outcome = "sent";
        let keys = taken.entries.iter().map(|entry| entry.key.clone());
        flushed.taken.extend(keys);
        let settled = settle::send(outbox, credential, &taken.entries, wait, clock);
        flushed.sent += settled.sent;
        flushed.dead += settled.dead;
        flushed.put_off.extend(settled.later);
        flushed.failed.extend(settled.failed);
        flushed.not_before = settled.not_before;
    }
    if !flushed.failed.is_empty() {
        flushed.outcome = "failed";
    } else if flushed.not_before.is_some() {
        flushed.outcome = "paused";
    }
    super::left(flushed, outbox)
}
