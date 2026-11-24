//! Connected mode (the Connected mode Feature Group): with `LOFT_YARD_SERVER_ENDPOINT` and
//! `LOFT_YARD_SERVER_TOKEN` both set, each version an agent writes is copied into the outbox and
//! sent to Yard Server from there. One sending serves every caller: it takes the oldest entries
//! of the outbox for its address, at most 16, sends them as one batch of versions through the
//! network client, waits at most 2 seconds for the answer, and removes the entries once Yard
//! Server has taken them. `loft yard write`, `loft yard task` and `loft yard published` run it
//! before they exit, once their result is printed and the lock of Yard Log is released, where
//! their write put entries; `loft yard flush` runs it by hand. The hooks and the page server
//! never send.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use yardlog::batch::{MOST_RECORDS, batch};
use yardlog_store::home::Home;
use yardlog_store::outbox::{Outbox, Taken};

use super::args::FlushArgs;
use crate::finish::Finished;
use crate::json;
use crate::network;
use crate::out::{errln, outln};

const SCHEMA: &str = "loft-yard-flush/1";

/// The variable that names Yard Server's base address.
pub const ENDPOINT: &str = "LOFT_YARD_SERVER_ENDPOINT";

/// The variable that names the token Yard Server admits the person by.
pub const TOKEN: &str = "LOFT_YARD_SERVER_TOKEN";

/// The most a sending waits for Yard Server's answer.
const WAIT: Duration = Duration::from_secs(2);

/// The path that takes a batch of versions, under Yard Server's address.
const BATCHES: &str = "batches";

/// Whether a write of this run put entries into the outbox.
static PUT: AtomicBool = AtomicBool::new(false);

/// Yard Server's address and token, where connected mode is on: both variables set and not
/// empty.
pub fn connected() -> Option<(String, String)> {
    let variable = |name| std::env::var(name).ok().filter(|value| !value.is_empty());
    Some((variable(ENDPOINT)?, variable(TOKEN)?))
}

/// The outbox of the Yard Server connected mode names, where it is on and a home is known.
pub fn outbox() -> Option<Outbox> {
    let (endpoint, _) = connected()?;
    Some(Home::from_env()?.outbox(&endpoint))
}

/// Notes that a write of this run put `count` entries into the outbox.
pub fn noted(count: usize) {
    if count > 0 {
        PUT.store(true, Ordering::SeqCst);
    }
}

/// What one sending did.
struct Flushed {
    outcome: &'static str,
    taken: Taken,
    sent: usize,
    /// Why the batch did not go, where it did not.
    failed: Option<String>,
}

impl Flushed {
    fn fields(&self) -> Value {
        json!({
            "sent": self.sent,
            "remaining": self.taken.waiting - self.sent,
            "skipped": self.taken.skipped,
            "dead": self.taken.dead,
        })
    }
}

/// Sends one batch of the oldest entries of `outbox` to Yard Server with `credential`, its token.
fn once(outbox: &Outbox, credential: &str) -> Flushed {
    let taken = match outbox.take(MOST_RECORDS) {
        Ok(taken) => taken,
        Err(error) => return failed(Taken::default(), error.to_string()),
    };
    if taken.entries.is_empty() {
        return Flushed {
            outcome: "empty",
            taken,
            sent: 0,
            failed: None,
        };
    }
    let records: Vec<&str> = taken
        .entries
        .iter()
        .map(|entry| entry.record.as_str())
        .collect();
    let body = batch(&records);
    let url = format!("{}/{BATCHES}", outbox.endpoint().trim_end_matches('/'));
    let bearer = format!("Bearer {credential}");
    let headers = [
        ("Authorization", bearer.as_str()),
        ("Content-Type", "application/json"),
    ];
    match network::call("POST", &url, &headers, Some(body.as_bytes()), WAIT) {
        Ok(reply) if reply.status == 200 => match outbox.remove(&taken.entries) {
            Ok(()) => Flushed {
                outcome: "sent",
                sent: taken.entries.len(),
                taken,
                failed: None,
            },
            Err(error) => failed(taken, error.to_string()),
        },
        Ok(reply) => {
            let why = refusal(reply.status, &reply.body);
            failed(taken, why)
        }
        Err(error) => failed(taken, format!("Yard Server did not answer: {error}")),
    }
}

fn failed(taken: Taken, why: String) -> Flushed {
    Flushed {
        outcome: "failed",
        taken,
        sent: 0,
        failed: Some(why),
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

/// Sends one batch where connected mode is on and a write of this run put entries into the
/// outbox: what does not go is named on standard error, and changes nothing else of the run.
pub fn after_write() {
    if !PUT.load(Ordering::SeqCst) {
        return;
    }
    let (Some(outbox), Some((_, token))) = (outbox(), connected()) else {
        return;
    };
    let flushed = once(&outbox, &token);
    if let Some(why) = &flushed.failed {
        errln!(
            "The versions did not reach Yard Server at {}: {why}; they stay in the outbox for the next write or `loft yard flush`.",
            outbox.endpoint()
        );
    }
    unsendable(&flushed.taken, &outbox);
}

/// Names on standard error the entries passed over and the files moved into `dead/`.
fn unsendable(taken: &Taken, outbox: &Outbox) {
    if taken.skipped > 0 {
        errln!(
            "{} entries of the outbox are of a newer format and wait for a newer `loft`.",
            taken.skipped
        );
    }
    if taken.dead > 0 {
        errln!(
            "{} files of the outbox were not entries and were moved into {}.",
            taken.dead,
            outbox.dir().join(yardlog_store::outbox::DEAD).display()
        );
    }
}

/// `loft yard flush`: sends one batch of versions from the outbox to Yard Server, as a writing
/// command does before it exits. It needs no repository, takes no session and writes nothing of
/// Yard Log.
pub fn run(args: &FlushArgs) -> Finished {
    let as_json = args.output.json();
    let (Some(outbox), Some((_, token))) = (outbox(), connected()) else {
        let fields = json!({ "sent": 0, "remaining": 0, "skipped": 0, "dead": 0 });
        return report(as_json, "off", fields, Vec::new(), "Connected mode is off.");
    };
    let flushed = once(&outbox, &token);
    let fields = flushed.fields();
    let line = format!(
        "{} to Yard Server at {}: {} sent, {} remaining, {} skipped, {} moved into dead/.",
        flushed.outcome,
        outbox.endpoint(),
        fields["sent"],
        fields["remaining"],
        fields["skipped"],
        fields["dead"]
    );
    let errors = flushed
        .failed
        .iter()
        .map(|why| json::error_at("yard-server", why, Vec::new()))
        .collect();
    report(as_json, flushed.outcome, fields, errors, &line)
}

fn report(
    as_json: bool,
    outcome: &'static str,
    fields: Value,
    errors: Vec<Value>,
    line: &str,
) -> Finished {
    let finished = if errors.is_empty() {
        Finished::success(outcome)
    } else {
        Finished::with(std::process::ExitCode::FAILURE, outcome, &errors)
    };
    if as_json {
        json::print(&json::result(SCHEMA, outcome, fields, errors));
    } else if finished.code == std::process::ExitCode::SUCCESS {
        outln!("{line}");
    } else {
        errln!("{line}");
    }
    finished
}
