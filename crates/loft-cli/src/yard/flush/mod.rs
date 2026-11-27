//! Connected mode (the Connected mode Feature Group): with `LOFT_YARD_SERVER_ENDPOINT` and
//! `LOFT_YARD_SERVER_TOKEN` both set, each version an agent writes is copied into the outbox, and
//! `loft yard flush` sends it to Yard Server from there. The writing commands, the hooks and the
//! page server never send; the session of an agent host runs the flush in the background.
//!
//! One flush sends at a time for each home of Yard, under the lock `~/.yard/flush.lock`, taken
//! without waiting: a flush that finds it held ends at once as `busy`. A write never takes it.
//! Holding it, a flush first keeps away from an address Yard Server asked it to come back to
//! later, as `~/.yard/flush.json` records, and records each run it makes there.

pub(super) mod record;
mod sending;
mod settle;
#[cfg(test)]
mod tests;

use std::time::Duration;

use loft_ports::clock::{Clock as _, SystemClock};
use serde_json::{Value, json};
use yardlog::Time;
use yardlog_store::home::{Home, HomeFile};
use yardlog_store::lock;
use yardlog_store::outbox::{self, Outbox};

use super::args::FlushArgs;
use super::failure;
use crate::environment;
use crate::finish::Finished;
use crate::json;
use crate::out::{errln, outln};
use record::Record;
use sending::Flushed;
use settle::Failed;

const SCHEMA: &str = "loft-yard-flush/1";

/// The variable that names Yard Server's base address.
pub const ENDPOINT: &str = "LOFT_YARD_SERVER_ENDPOINT";

/// The variable that names the token Yard Server admits the person by.
pub const TOKEN: &str = "LOFT_YARD_SERVER_TOKEN";

/// The variable that names how many seconds a flush waits for Yard Server's answer to a batch.
pub const TIMEOUT: &str = "LOFT_YARD_SERVER_TIMEOUT";

/// The lock a flush holds while it sends, in the home of Yard.
pub const LOCK: &str = "flush.lock";

/// The most a flush waits for Yard Server's answer to one batch, unless [`TIMEOUT`] says.
const WAIT: Duration = Duration::from_secs(10);

/// Where connected mode sends: the home of Yard, the outbox of the address and the token.
pub(in crate::yard) struct Connection {
    pub home: Home,
    pub outbox: Outbox,
    token: String,
}

/// Whether connected mode is on, as the environment says.
pub(in crate::yard) enum Mode {
    /// A variable is unset or empty: each that is.
    Off(Vec<&'static str>),
    /// Both variables are set, and no home directory is known to keep the outbox in.
    Homeless,
    On(Connection),
}

pub(in crate::yard) fn mode() -> Mode {
    let (Some(endpoint), Some(token)) = (environment::text(ENDPOINT), environment::text(TOKEN))
    else {
        let missing = [ENDPOINT, TOKEN]
            .into_iter()
            .filter(|name| environment::text(name).is_none())
            .collect();
        return Mode::Off(missing);
    };
    let Some(home) = Home::from_env() else {
        return Mode::Homeless;
    };
    let outbox = home.outbox(&endpoint);
    Mode::On(Connection {
        home,
        outbox,
        token,
    })
}

/// The outbox each new version is copied into, where connected mode is on.
pub fn outbox() -> Option<Outbox> {
    match mode() {
        Mode::On(connection) => Some(connection.outbox),
        Mode::Off(_) | Mode::Homeless => None,
    }
}

/// Whether a flush has work it may do at `now`, in seconds since the Unix epoch: connected mode
/// is on, the outbox holds an entry for its address that the last run did not record as one Yard
/// Server will take later, and Yard Server asked to come back after no time that has not come.
pub(in crate::yard) fn due(mode: &Mode, now: u64) -> bool {
    let Mode::On(connection) = mode else {
        return false;
    };
    let last = record::read(&connection.home, connection.outbox.endpoint());
    let paused = last.as_ref().and_then(|last| last.paused(now)).is_some();
    !paused && aside(Flushed::quiet("due"), &connection.outbox, last).remaining() > 0
}

/// How long a flush waits for an answer: `seconds` where [`TIMEOUT`] gives them, else 10 seconds.
fn wait_of(seconds: Option<u64>) -> Duration {
    seconds.map_or(WAIT, Duration::from_secs)
}

/// How long this flush waits for an answer, or why [`TIMEOUT`] does not say.
fn wait() -> Result<Duration, String> {
    crate::remote::seconds(TIMEOUT)
        .map(wait_of)
        .map_err(|error| error.to_string())
}

/// `loft yard flush`: sends the outbox's entries for its address to Yard Server, batch after
/// batch, under the lock of the flush. It needs no repository, takes no session and writes
/// nothing of Yard Log.
pub fn run(args: &FlushArgs) -> Finished {
    let as_json = args.output.json();
    let connection = match mode() {
        Mode::Off(_) => return report(as_json, &Flushed::quiet("off"), "Connected mode is off."),
        Mode::Homeless => {
            let why = format!(
                "{ENDPOINT} and {TOKEN} are set, but no home directory is known to keep the outbox in"
            );
            let line = format!("failed: {why}.");
            return report(as_json, &Flushed::failed(Failed::outbox(why)), &line);
        }
        Mode::On(connection) => connection,
    };
    let endpoint = connection.outbox.endpoint().to_owned();
    let unlocked = |flushed| {
        let last = record::read(&connection.home, &endpoint);
        aside(flushed, &connection.outbox, last)
    };
    let flushed = match lock::try_lock(&connection.home.dir().join(LOCK)) {
        Ok(Some(_held)) => locked(&connection),
        Ok(None) => {
            let busy = unlocked(Flushed::quiet("busy"));
            let line = format!(
                "busy: another flush sends to Yard Server at {endpoint}; {} remaining.",
                busy.remaining()
            );
            return report(as_json, &busy, &line);
        }
        Err(error) => unlocked(Flushed::failed(Failed::outbox(format!(
            "the lock of the flush cannot be taken: {error}"
        )))),
    };
    let mut line = format!(
        "{} to Yard Server at {endpoint}: {} sent, {} remaining, {} later, {} skipped, {} moved into dead/.",
        flushed.outcome,
        flushed.sent,
        flushed.remaining(),
        flushed.later(),
        flushed.skipped,
        flushed.dead
    );
    if let Some(time) = flushed
        .not_before
        .and_then(|time| Time::from_unix_seconds(time).ok())
    {
        line = format!("{line} Yard Server asked to come back after {time}.");
    }
    for failed in &flushed.failed {
        line = format!("{line} {}.", failed.why);
    }
    report(as_json, &flushed, &line)
}

/// The run of a flush that holds the lock: none where Yard Server asked to come back after a
/// time that has not come, else the sending, recorded in `~/.yard/flush.json`.
fn locked(connection: &Connection) -> Flushed {
    // The system's clock, which `SOURCE_DATE_EPOCH` does not pin, as `crate::remote::now` reads
    // it, so that a pause Yard Server asks for ends in real time.
    let clock = &SystemClock::default();
    let endpoint = connection.outbox.endpoint();
    let now = clock.now().unwrap_or_default();
    let recorded = record::read(&connection.home, endpoint).unwrap_or_default();
    if let Some(time) = recorded.paused(now) {
        let paused = Flushed {
            not_before: Some(time),
            ..Flushed::quiet("paused")
        };
        return aside(paused, &connection.outbox, Some(recorded));
    }
    let mut flushed = match wait() {
        Ok(wait) => sending::drain(&connection.outbox, &connection.token, wait, clock),
        Err(why) => left(
            Flushed::failed(Failed::yard_server(why)),
            &connection.outbox,
        ),
    };
    let run = record::Run {
        ended: clock.now().unwrap_or(now),
        outcome: flushed.outcome,
        why: flushed.failed.first().map(|failed| failed.why.clone()),
        not_before: flushed.not_before,
        later: &flushed.put_off,
    };
    match record::replace(&connection.home, endpoint, &run, &flushed.waiting) {
        Ok(written) => flushed.held = written.later_keys(),
        Err(error) => {
            flushed.failed.push(Failed::outbox(format!(
                "{} cannot be written: {error}",
                connection.home.path(HomeFile::Flush).display()
            )));
            flushed.outcome = "failed";
        }
    }
    flushed
}

/// `flushed` with what is left in `outbox` after it: the keys of the entries for its address,
/// and the entries of a newer format, passed over.
fn left(flushed: Flushed, outbox: &Outbox) -> Flushed {
    let mut survey = outbox::survey(outbox.dir()).unwrap_or_default();
    let waiting = survey.waiting.remove(outbox.endpoint()).unwrap_or_default();
    Flushed {
        waiting: waiting.into_iter().collect(),
        skipped: survey.newer,
        ..flushed
    }
}

/// `flushed`, of a run that sent nothing, with what is left in `outbox` and the keys `last`, the
/// record of the address's last run, holds as later.
fn aside(flushed: Flushed, outbox: &Outbox, last: Option<Record>) -> Flushed {
    let held = last.map(Record::later_keys).unwrap_or_default();
    left(Flushed { held, ..flushed }, outbox)
}

fn report(as_json: bool, flushed: &Flushed, line: &str) -> Finished {
    let fields = json!({
        "sent": flushed.sent,
        "remaining": flushed.remaining(),
        "later": flushed.later(),
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
