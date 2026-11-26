//! The sending (the Send versions to Yard Server Feature): one run takes the oldest entries of
//! the outbox for its address, at most 16 and at most 1 MiB of them, sends them as one batch of
//! versions through the network client, and takes the next batch, until no entry is left that it
//! has not taken in this run, a batch does not go, or 5 minutes have passed. Entries put into the
//! outbox while a batch is on its way go in a later batch of the same run.

use std::collections::BTreeSet;
use std::time::Duration;

use loft_ports::clock::Clock;
use serde_json::Value;
use yardlog::batch::{MOST_BYTES, MOST_RECORDS, batch};
use yardlog_store::outbox::{self, Entry, Outbox};

use crate::network;

/// The most a sending waits for Yard Server's answer to one batch.
const WAIT: Duration = Duration::from_secs(2);

/// How long a run takes new batches: after it, the run ends once the batch on its way is settled.
pub(super) const MOST_TIME: Duration = Duration::from_mins(5);

/// The path that takes a batch of versions, under Yard Server's address.
const BATCHES: &str = "batches";

/// What one run did.
#[derive(Debug)]
pub(super) struct Flushed {
    pub outcome: &'static str,
    /// The versions Yard Server took, in every batch of the run.
    pub sent: usize,
    /// The entries for the address left in the outbox.
    pub remaining: usize,
    /// The entries of a newer format passed over.
    pub skipped: usize,
    /// The files moved into `dead/` in the run.
    pub dead: usize,
    /// Why a batch did not go, where one did not.
    pub failed: Option<String>,
}

impl Flushed {
    /// The result of a run that did not look at the outbox.
    pub fn quiet(outcome: &'static str) -> Self {
        Self {
            outcome,
            sent: 0,
            remaining: 0,
            skipped: 0,
            dead: 0,
            failed: None,
        }
    }

    pub fn failed(why: String) -> Self {
        Self {
            failed: Some(why),
            ..Self::quiet("failed")
        }
    }
}

/// Runs the sending of `outbox` with `credential`, its token, reading the time from `clock`.
pub(super) fn drain(outbox: &Outbox, credential: &str, clock: &dyn Clock) -> Flushed {
    let began = clock.elapsed();
    let mut flushed = Flushed::quiet("empty");
    let mut passed = BTreeSet::new();
    while clock.elapsed().saturating_sub(began) < MOST_TIME {
        let taken = match outbox.take_after(MOST_RECORDS, MOST_BYTES, &passed) {
            Ok(taken) => taken,
            Err(error) => {
                flushed.failed = Some(error.to_string());
                break;
            }
        };
        flushed.dead += taken.dead;
        flushed.skipped = taken.skipped;
        if taken.entries.is_empty() {
            break;
        }
        flushed.outcome = "sent";
        passed.extend(taken.entries.iter().map(|entry| entry.key.clone()));
        match send(outbox, credential, &taken.entries) {
            Ok(sent) => flushed.sent += sent,
            Err(why) => {
                flushed.failed = Some(why);
                break;
            }
        }
    }
    if flushed.failed.is_some() {
        flushed.outcome = "failed";
    }
    flushed.remaining = outbox::survey(outbox.dir())
        .ok()
        .and_then(|survey| survey.waiting.get(outbox.endpoint()).map(Vec::len))
        .unwrap_or(0);
    flushed
}

/// Sends `entries` as one batch: how many versions Yard Server took, or why the batch did not go.
fn send(outbox: &Outbox, credential: &str, entries: &[Entry]) -> Result<usize, String> {
    let records: Vec<&str> = entries.iter().map(|entry| entry.record.as_str()).collect();
    let body = batch(&records);
    let url = format!("{}/{BATCHES}", outbox.endpoint().trim_end_matches('/'));
    let bearer = format!("Bearer {credential}");
    let headers = [
        ("Authorization", bearer.as_str()),
        ("Content-Type", "application/json"),
    ];
    match network::call("POST", &url, &headers, Some(body.as_bytes()), WAIT) {
        Ok(reply) if reply.status == 200 => match outbox.remove(entries) {
            Ok(()) => Ok(entries.len()),
            Err(error) => Err(format!(
                "Yard Server took the batch, but the outbox could not drop it: {error}"
            )),
        },
        Ok(reply) => Err(refusal(reply.status, &reply.body)),
        Err(error) => Err(format!("Yard Server did not answer: {error}")),
    }
}

/// A refusal of Yard Server's: its status, and its code where it is a problem document.
fn refusal(status: u16, body: &[u8]) -> String {
    let code = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|problem| problem["code"].as_str().map(str::to_owned));
    match code {
        Some(code) => format!("Yard Server refused the batch with {status} `{code}`"),
        None => format!("Yard Server answered the batch with {status}"),
    }
}
