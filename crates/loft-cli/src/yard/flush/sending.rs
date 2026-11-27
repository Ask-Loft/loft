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

use super::settle::{Failed, Sender, Settled};

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
    /// The versions Yard Server will take later, by key, each with its code.
    pub put_off: BTreeMap<String, String>,
    /// The keys of the entries for the address left in the outbox, put off ones included.
    pub waiting: BTreeSet<String>,
    /// The keys `~/.yard/flush.json` records as those of versions Yard Server will take later.
    pub held: BTreeSet<String>,
    /// The time a `Retry-After` of the run asked it not to come back before, in seconds.
    pub retry_after: Option<u64>,
    /// When Yard Server last answered a version `later` in the run, in seconds.
    pub later_at: Option<u64>,
    /// Whether a batch of the run went.
    pub went: bool,
    /// The time before which no run makes a request, in seconds, as recorded once the run ended,
    /// where it has not passed.
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
            put_off: BTreeMap::new(),
            waiting: BTreeSet::new(),
            held: BTreeSet::new(),
            retry_after: None,
            later_at: None,
            went: false,
            not_before: None,
        }
    }

    /// The result of a run that ended `outcome` after its batches came to `settled`.
    fn of(outcome: &'static str, settled: Settled) -> Self {
        Self {
            sent: settled.sent,
            dead: settled.dead,
            failed: settled.failed,
            put_off: settled.later.into_iter().collect(),
            retry_after: settled.retry_after,
            later_at: settled.later_at,
            went: settled.went,
            ..Self::quiet(outcome)
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
/// answer, reading the time from `clock` and passing over the entries whose keys `deferred`
/// holds.
pub(super) fn drain(
    outbox: &Outbox,
    credential: &str,
    wait: Duration,
    clock: &dyn Clock,
    deferred: BTreeSet<String>,
) -> Flushed {
    let sender = Sender {
        outbox,
        credential,
        wait,
        clock,
        until: clock.elapsed().saturating_add(MOST_TIME),
    };
    // The entries the run passes over: those put off, and each it has taken once, whatever became
    // of it.
    let mut passed = deferred;
    let mut settled = Settled::default();
    let mut outcome = "empty";
    while settled.failed.is_empty()
        && settled.retry_after.is_none()
        && clock.elapsed() < sender.until
    {
        let taken = match outbox.take_after(MOST_RECORDS, MOST_BYTES, &passed) {
            Ok(taken) => taken,
            Err(error) => {
                settled.failed.push(Failed::outbox(error.to_string()));
                break;
            }
        };
        settled.dead += taken.dead;
        if taken.entries.is_empty() {
            break;
        }
        outcome = "sent";
        passed.extend(taken.entries.iter().map(|entry| entry.key.clone()));
        settled.add(sender.send(&taken.entries));
    }
    if !settled.failed.is_empty() {
        outcome = "failed";
    } else if settled.retry_after.is_some() {
        outcome = "paused";
    }
    super::left(Flushed::of(outcome, settled), outbox)
}
