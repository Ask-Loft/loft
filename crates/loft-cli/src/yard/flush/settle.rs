//! One batch of the sending: sent as `POST <endpoint>/batches`, and each of its entries settled
//! by Yard Server's answer. A version Yard Server stored or already held leaves the outbox; one
//! it holds other contents for, or refused, is moved into `dead/` with the reason, as is every
//! entry of a batch it refused whole with `400` or `413` and its own code; one it will take later
//! stays. A `400` or `413` without that code comes from something on the way, such as a proxy,
//! and settles nothing; a `413` to a batch of several versions splits it in halves, each sent as
//! a batch of its own, the older first. An
//! answer of `200` that does not read, or gives a version no outcome this `loft` knows, settles
//! nothing of that version, and the run ends `failed`. A `429` or `503` whose `Retry-After` reads
//! as a time that can be dated asks the run to come back later, and settles nothing.

use std::slice;
use std::time::Duration;

use loft_ports::clock::Clock;
use serde_json::Value;
use yardlog::Time;
use yardlog::batch::batch;
use yardlog::batch_answer::{self, Outcome};
use yardlog_store::outbox::{Entry, Outbox};

use crate::network;

/// The path that takes a batch of versions, under Yard Server's address.
const BATCHES: &str = "batches";

/// The longest a `Retry-After` keeps the sending away, in seconds.
const MOST_PAUSE: u64 = 60 * 60;

/// The code of Yard Server's own refusal of a batch with `status`, where it refuses one for good.
fn own_code(status: u16) -> &'static str {
    if status == 413 {
        "payload_too_large"
    } else {
        "invalid_request"
    }
}

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
    /// The time Yard Server asked the run not to come back before with a `Retry-After`, in
    /// seconds since the Unix epoch, where it refused the batch for now.
    pub retry_after: Option<u64>,
    /// When Yard Server last answered a version of the batch `later`, in seconds since the Unix
    /// epoch.
    pub later_at: Option<u64>,
    /// Whether the batch went: Yard Server answered it with a `200` that reads, or refused it
    /// for good with its own code.
    pub went: bool,
    /// Why the batch did not go, or an entry of it could not be settled.
    pub failed: Vec<Failed>,
}

/// Why the network client refuses `outbox`'s address, where it does: then no batch is sent.
pub(super) fn refused(outbox: &Outbox) -> Option<String> {
    let url = format!("{}/{BATCHES}", outbox.endpoint());
    network::allowed(&url).is_none().then(|| {
        format!(
            "the address {} is refused: {}",
            outbox.endpoint(),
            network::CONTACTED
        )
    })
}

/// What sends the batches of one run to the address of `outbox`, which the network client
/// accepts, with `credential`.
pub(super) struct Sender<'a> {
    pub outbox: &'a Outbox,
    pub credential: &'a str,
    /// The most it waits for each answer.
    pub wait: Duration,
    pub clock: &'a dyn Clock,
    /// The time, as `clock` counts it, past which the halves of a batch split after a `413` are
    /// no longer sent.
    pub until: Duration,
}

impl Sender<'_> {
    /// Sends `entries` as one batch and settles each entry by the answer; a `Retry-After` counts
    /// from the time the clock gives once it answered.
    pub fn send(&self, entries: &[Entry]) -> Settled {
        let records: Vec<&str> = entries.iter().map(|entry| entry.record.as_str()).collect();
        let body = batch(&records);
        let url = format!("{}/{BATCHES}", self.outbox.endpoint());
        let bearer = format!("Bearer {}", self.credential);
        let headers = [
            ("Authorization", bearer.as_str()),
            ("Content-Type", "application/json"),
        ];
        match network::call("POST", &url, &headers, Some(body.as_bytes()), self.wait) {
            Ok(reply) => self.answered(entries, &reply),
            Err(error) => failure(Failed::yard_server(format!(
                "Yard Server did not answer: {error}"
            ))),
        }
    }

    /// Sends the halves of a batch Yard Server answered `413`, the older first, each while time
    /// is left.
    fn split(&self, entries: &[Entry]) -> Settled {
        let (older, newer) = entries.split_at(entries.len() / 2);
        let mut settled = Settled::default();
        for half in [older, newer] {
            if !settled.failed.is_empty()
                || settled.retry_after.is_some()
                || self.clock.elapsed() >= self.until
            {
                break;
            }
            settled.add(self.send(half));
        }
        settled
    }

    /// Settles `entries` by `reply`, Yard Server's answer to their batch.
    fn answered(&self, entries: &[Entry], reply: &network::Reply) -> Settled {
        let (outbox, clock) = (self.outbox, self.clock);
        match reply.status {
            200 => {
                let mut settled = settle(outbox, entries, &reply.body);
                if !settled.later.is_empty() {
                    settled.later_at = clock.now().ok();
                }
                settled
            }
            413 if entries.len() > 1 => self.split(entries),
            400 | 413 => {
                let expected = own_code(reply.status);
                if code(&reply.body).as_deref() != Some(expected) {
                    return failure(Failed::yard_server(format!(
                        "the batch was answered {} by something that is not Yard Server: the answer \
                     carries no code `{expected}`",
                        reply.status
                    )));
                }
                let reason = format!("{} {expected}", reply.status);
                let mut settled = Settled {
                    went: true,
                    ..Settled::default()
                };
                for entry in entries {
                    bury(outbox, entry, &reason, &mut settled);
                }
                settled
            }
            429 | 503 => match reply.retry_after.as_deref().and_then(|value| {
                let now = clock.now().unwrap_or_default();
                let seconds = crate::remote::retry_after(value, now)?.min(MOST_PAUSE);
                Some(now.saturating_add(seconds))
                    .filter(|time| Time::from_unix_seconds(*time).is_ok())
            }) {
                Some(time) => Settled {
                    retry_after: Some(time),
                    ..Settled::default()
                },
                None => failure(Failed::yard_server(refusal(reply.status, &reply.body))),
            },
            status => failure(Failed::yard_server(refusal(status, &reply.body))),
        }
    }
}

impl Settled {
    /// Adds what became of a later batch of the same run, sent while no batch before it had
    /// failed or been asked to come back later.
    pub fn add(&mut self, other: Self) {
        self.sent += other.sent;
        self.dead += other.dead;
        self.later.extend(other.later);
        self.retry_after = other.retry_after;
        self.later_at = other.later_at.or(self.later_at);
        self.failed.extend(other.failed);
        self.went |= other.went;
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
    let mut settled = Settled {
        went: true,
        ..Settled::default()
    };
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
