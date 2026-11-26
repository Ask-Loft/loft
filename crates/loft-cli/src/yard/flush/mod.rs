//! Connected mode (the Connected mode Feature Group): with `LOFT_YARD_SERVER_ENDPOINT` and
//! `LOFT_YARD_SERVER_TOKEN` both set, each version an agent writes is copied into the outbox, and
//! `loft yard flush` sends it to Yard Server from there. The writing commands, the hooks and the
//! page server never send; the session of an agent host runs the flush in the background.
//!
//! One flush sends at a time for each home of Yard, under the lock `~/.yard/flush.lock`, taken
//! without waiting: a flush that finds it held ends at once as `busy`. A write never takes it.

mod sending;
mod settle;
#[cfg(test)]
mod tests;

use std::time::Duration;

use serde_json::{Value, json};
use yardlog_store::home::Home;
use yardlog_store::lock;
use yardlog_store::outbox::Outbox;

use super::args::FlushArgs;
use super::failure;
use crate::environment;
use crate::finish::Finished;
use crate::json;
use crate::out::{errln, outln};
use sending::Flushed;
use settle::Failed;

const SCHEMA: &str = "loft-yard-flush/1";

/// The variable that names Yard Server's base address.
pub const ENDPOINT: &str = "LOFT_YARD_SERVER_ENDPOINT";

/// The variable that names the token Yard Server admits the person by.
pub const TOKEN: &str = "LOFT_YARD_SERVER_TOKEN";

/// The lock a flush holds while it sends, in the home of Yard.
pub const LOCK: &str = "flush.lock";

/// The most a flush waits for Yard Server's answer to one batch.
const WAIT: Duration = Duration::from_secs(2);

/// Where connected mode sends: the home of Yard, the outbox of the address and the token.
struct Connection {
    home: Home,
    outbox: Outbox,
    token: String,
}

/// The connection connected mode names, where it is on, both variables set and not empty, and a
/// home is known.
fn connection() -> Option<Connection> {
    let endpoint = environment::text(ENDPOINT)?;
    let token = environment::text(TOKEN)?;
    let home = Home::from_env()?;
    let outbox = home.outbox(&endpoint);
    Some(Connection {
        home,
        outbox,
        token,
    })
}

/// The outbox each new version is copied into, where connected mode is on.
pub fn outbox() -> Option<Outbox> {
    connection().map(|connection| connection.outbox)
}

/// `loft yard flush`: sends the outbox's entries for its address to Yard Server, batch after
/// batch, under the lock of the flush. It needs no repository, takes no session and writes
/// nothing of Yard Log.
pub fn run(args: &FlushArgs) -> Finished {
    let as_json = args.output.json();
    let Some(connection) = connection() else {
        return report(as_json, &Flushed::quiet("off"), "Connected mode is off.");
    };
    let endpoint = connection.outbox.endpoint().to_owned();
    let path = connection.home.dir().join(LOCK);
    let flushed = match lock::try_lock(&path) {
        Ok(Some(_held)) => {
            sending::drain(&connection.outbox, &connection.token, WAIT, super::clock())
        }
        Ok(None) => {
            let line = format!("busy: another flush sends to Yard Server at {endpoint}.");
            return report(as_json, &Flushed::quiet("busy"), &line);
        }
        Err(error) => Flushed::failed(Failed::outbox(format!(
            "the lock of the flush cannot be taken: {error}"
        ))),
    };
    let mut line = format!(
        "{} to Yard Server at {endpoint}: {} sent, {} remaining, {} later, {} skipped, {} moved into dead/.",
        flushed.outcome,
        flushed.sent,
        flushed.remaining,
        flushed.later,
        flushed.skipped,
        flushed.dead
    );
    for failed in &flushed.failed {
        line = format!("{line} {}.", failed.why);
    }
    report(as_json, &flushed, &line)
}

fn report(as_json: bool, flushed: &Flushed, line: &str) -> Finished {
    let fields = json!({
        "sent": flushed.sent,
        "remaining": flushed.remaining,
        "later": flushed.later,
        "skipped": flushed.skipped,
        "dead": flushed.dead,
    });
    let errors: Vec<Value> = flushed
        .failed
        .iter()
        .map(|failed| failure::error(failed.kind, &failed.why))
        .collect();
    let outcome = flushed.outcome;
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
