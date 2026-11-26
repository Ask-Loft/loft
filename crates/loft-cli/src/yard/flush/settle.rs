//! One batch of the sending: sent as `POST <endpoint>/batches`, and each of its entries settled
//! by Yard Server's answer. A version Yard Server stored or already held leaves the outbox; one
//! it holds other contents for, or refused, is moved into `dead/` with the reason, as is every
//! entry of a batch it refused whole with `400` or `413`; one it will take later stays. An
//! answer of `200` that does not read, or gives a version no outcome this `loft` knows, settles
//! nothing of that version, and the run ends `failed`. A `429` or `503` whose `Retry-After` reads
//! as a time that can be dated asks the run to come back later, and settles nothing.

use std::slice;
use std::time::Duration;

use serde_json::Value;
use yardlog::Time;
use yardlog::batch::batch;
use yardlog::batch_answer::{self, Outcome};
use yardlog_store::outbox::{Entry, Outbox};

use crate::network;

/// The path that takes a batch of versions, under Yard Server's address.
const BATCHES: &str = "batches";

/// Why a run ends `failed`: the kind of its error, `yard-server` or `outbox`, and what it says.
#[derive(Debug)]
pub(super) struct Failed {
    pub kind: &'static str,
    pub why: String,
}

impl Failed {
    pub fn yard_server(why: String) -> Self {
        Self {
            kind: "yard-server",
            why,
        }
    }

    pub fn outbox(why: String) -> Self {
        Self {
            kind: "outbox",
            why,
        }
    }
}

/// What became of one batch.
#[derive(Debug, Default)]
pub(super) struct Settled {
    /// The versions Yard Server stored or already held.
    pub sent: usize,
    /// The entries moved into `dead/`.
    pub dead: usize,
    /// The keys of the entries whose versions Yard Server will take later, each with its code.
    pub later: Vec<(String, String)>,
    /// The seconds Yard Server asked the run to keep away, where it refused the batch for now.
    pub come_back: Option<u64>,
    /// Why the batch did not go, or an entry of it could not be settled.
    pub failed: Vec<Failed>,
}

/// Sends `entries` as one batch with `credential`, waiting at most `wait` for the answer, and
/// settles each entry by it; `now` dates a `Retry-After` given as an HTTP date.
pub(super) fn send(
    outbox: &Outbox,
    credential: &str,
    entries: &[Entry],
    wait: Duration,
    now: u64,
) -> Settled {
    let records: Vec<&str> = entries.iter().map(|entry| entry.record.as_str()).collect();
    let body = batch(&records);
    let url = format!("{}/{BATCHES}", outbox.endpoint().trim_end_matches('/'));
    let bearer = format!("Bearer {credential}");
    let headers = [
        ("Authorization", bearer.as_str()),
        ("Content-Type", "application/json"),
    ];
    let reply = match network::call("POST", &url, &headers, Some(body.as_bytes()), wait) {
        Ok(reply) => reply,
        Err(error) => {
            return failure(Failed::yard_server(format!(
                "Yard Server did not answer: {error}"
            )));
        }
    };
    match reply.status {
        200 => settle(outbox, entries, &reply.body),
        400 | 413 => {
            let reason = match code(&reply.body) {
                Some(code) => format!("{} {code}", reply.status),
                None => reply.status.to_string(),
            };
            let mut settled = Settled::default();
            for entry in entries {
                bury(outbox, entry, &reason, &mut settled);
            }
            settled
        }
        429 | 503 => match reply
            .retry_after
            .as_deref()
            .and_then(|value| crate::remote::retry_after(value, now))
            .filter(|seconds| Time::from_unix_seconds(now.saturating_add(*seconds)).is_ok())
        {
            Some(seconds) => Settled {
                come_back: Some(seconds),
                ..Settled::default()
            },
            None => failure(Failed::yard_server(refusal(reply.status, &reply.body))),
        },
        status => failure(Failed::yard_server(refusal(status, &reply.body))),
    }
}

fn failure(failed: Failed) -> Settled {
    Settled {
        failed: vec![failed],
        ..Settled::default()
    }
}

/// Settles each of `entries` by the outcome the answer `body` gives its version.
fn settle(outbox: &Outbox, entries: &[Entry], body: &[u8]) -> Settled {
    let Some(answer) = batch_answer::read(body) else {
        return failure(Failed::yard_server(
            "Yard Server answered the batch with 200, but its answer does not read".to_owned(),
        ));
    };
    let mut settled = Settled::default();
    let mut unsettled = Vec::new();
    for entry in entries {
        match answer.get(&entry.key) {
            Some(Outcome::Stored | Outcome::Held) => match outbox.remove(slice::from_ref(entry)) {
                Ok(()) => settled.sent += 1,
                Err(error) => settled.failed.push(Failed::outbox(format!(
                    "the outbox could not drop {}: {error}",
                    file(outbox, entry)
                ))),
            },
            Some(Outcome::Conflict) => bury(outbox, entry, "conflict", &mut settled),
            Some(Outcome::Refused(code)) => {
                bury(outbox, entry, &format!("refused {code}"), &mut settled);
            }
            Some(Outcome::Later(code)) => settled.later.push((entry.key.clone(), code.clone())),
            Some(Outcome::Unknown) | None => unsettled.push(entry.key.as_str()),
        }
    }
    if !unsettled.is_empty() {
        settled.failed.push(Failed::yard_server(format!(
            "Yard Server's answer gives no outcome this `loft` knows for version {}",
            unsettled.join(", ")
        )));
    }
    settled
}

/// Moves `entry` into `dead/` with `reason`, counting it in `settled`.
fn bury(outbox: &Outbox, entry: &Entry, reason: &str, settled: &mut Settled) {
    match outbox.bury(entry, reason) {
        Ok(moved) => settled.dead += usize::from(moved),
        Err(error) => settled.failed.push(Failed::outbox(format!(
            "the outbox could not move {} into dead/: {error}",
            file(outbox, entry)
        ))),
    }
}

/// The path of `entry`'s file.
fn file(outbox: &Outbox, entry: &Entry) -> String {
    outbox.path_of(entry).display().to_string()
}

/// The code of a problem document.
fn code(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|problem| problem["code"].as_str().map(str::to_owned))
}

/// A refusal of Yard Server's: its status, and its code where it is a problem document.
fn refusal(status: u16, body: &[u8]) -> String {
    match code(body) {
        Some(code) => format!("Yard Server refused the batch with {status} `{code}`"),
        None => format!("Yard Server answered the batch with {status}"),
    }
}
