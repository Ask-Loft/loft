//! `loft yard status` (the `loft yard status` Feature): whether the versions agents write leave
//! the machine, and where they do not, why. It reads the outbox, `~/.yard/outbox/dead/` and
//! `~/.yard/flush.json`, and nothing else: it makes no request and writes nothing. Each fact is one
//! line, a lowercase key, a colon and the fact, as `loft status` writes its lines; `--format json`
//! gives the same facts as one document. It exits 0 whatever it finds.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use loft_ports::clock::Clock as _;
use serde_json::{Map, Value, json};
use yardlog::Time;
use yardlog_store::home::Home;
use yardlog_store::outbox::{self, Survey};

use super::args::StatusArgs;
use super::flush::record::{self, Record};
use super::flush::{self, ENDPOINT, Mode, TOKEN};
use crate::finish::Finished;
use crate::json;
use crate::network;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-status/1";
const OUTCOME: &str = "reported";

/// Whether connected mode is on, and where it is not, why.
enum Connected {
    /// A variable is unset or empty: the variables missing.
    Off(Vec<&'static str>),
    /// Both variables are set, and no home directory is known.
    Homeless,
    /// On, sending to the address; whether the network client refuses it.
    On { endpoint: String, refused: bool },
}

/// What waits in the outbox.
struct Waiting {
    for_address: usize,
    other_addresses: usize,
    later: usize,
    skipped: usize,
    by_address: BTreeMap<String, usize>,
}

/// What `loft yard status` reports.
struct Report {
    connected: Connected,
    waiting: Waiting,
    /// The last run for the address, where connected mode is on and one is recorded.
    last: Option<Record>,
    dead: Vec<(String, String)>,
    /// The time now, in seconds since the Unix epoch.
    now: u64,
    /// Whether a home directory is known to keep `~/.yard/` in.
    home_known: bool,
}

pub fn run(args: &StatusArgs) -> Finished {
    let report = report();
    if args.output.json() {
        json::print(&json::result(SCHEMA, OUTCOME, fields(&report), Vec::new()));
    } else {
        for line in lines(&report) {
            outln!("{line}");
        }
    }
    Finished::success(OUTCOME)
}

fn report() -> Report {
    let now = super::clock().now().unwrap_or_default();
    let home = Home::from_env();
    let (connected, last) = match flush::mode() {
        Mode::Off(missing) => (Connected::Off(missing), None),
        Mode::Homeless => (Connected::Homeless, None),
        Mode::On(connection) => {
            let endpoint = connection.outbox.endpoint().to_owned();
            let last = record::read(&connection.home, &endpoint);
            let refused = network::allowed(&endpoint).is_none();
            (Connected::On { endpoint, refused }, last)
        }
    };
    let home_known = home.is_some();
    let dir = home.map(|home| home.outbox_dir());
    let survey = dir
        .as_deref()
        .and_then(|dir| outbox::survey(dir).ok())
        .unwrap_or_default();
    let dead = dir.as_deref().map(dead).unwrap_or_default();
    Report {
        waiting: waiting(&connected, &survey, last.as_ref()),
        connected,
        last,
        dead,
        now,
        home_known,
    }
}

/// What waits in the outbox, for the address where connected mode is on: an entry counts as put
/// off where the last run for the address recorded its version as `later`.
fn waiting(connected: &Connected, survey: &Survey, last: Option<&Record>) -> Waiting {
    let by_address: BTreeMap<String, usize> = survey
        .waiting
        .iter()
        .map(|(address, keys)| (address.clone(), keys.len()))
        .collect();
    let all: usize = by_address.values().sum();
    let ours: &[String] = match connected {
        Connected::On { endpoint, .. } => survey.waiting.get(endpoint).map_or(&[], Vec::as_slice),
        _ => &[],
    };
    let put_off: BTreeSet<&String> = last
        .map(|last| last.later.keys().collect())
        .unwrap_or_default();
    Waiting {
        for_address: ours.len(),
        other_addresses: all - ours.len(),
        later: ours.iter().filter(|key| put_off.contains(key)).count(),
        skipped: survey.newer,
        by_address,
    }
}

/// The files of `dead/` under `dir`, each with its reason, `unknown` where it has none.
fn dead(dir: &Path) -> Vec<(String, String)> {
    outbox::dead(dir)
        .unwrap_or_default()
        .into_iter()
        .map(|dead| {
            let reason = dead.reason.unwrap_or_else(|| "unknown".to_owned());
            (dead.file, reason)
        })
        .collect()
}

fn time(seconds: u64) -> String {
    Time::from_unix_seconds(seconds).map_or_else(|_| seconds.to_string(), |time| time.to_string())
}

fn lines(report: &Report) -> Vec<String> {
    let mut lines = vec![connected_line(&report.connected)];
    lines.push(waiting_line(&report.connected, &report.waiting));
    if let Connected::On { .. } = report.connected {
        lines.push(last_line(report.last.as_ref(), report.now));
    }
    if report.dead.is_empty() {
        lines.push("dead: none".to_owned());
    } else {
        lines.push(format!("dead: {}", report.dead.len()));
        lines.extend(
            report
                .dead
                .iter()
                .map(|(file, reason)| format!("dead {file}: {reason}")),
        );
    }
    lines
}

fn connected_line(connected: &Connected) -> String {
    match connected {
        Connected::Off(missing) if missing.len() == 1 => {
            format!("connected mode: off; {} is unset or empty", missing[0])
        }
        Connected::Off(_) => "connected mode: off".to_owned(),
        Connected::Homeless => {
            format!(
                "connected mode: off; {ENDPOINT} and {TOKEN} are set, but no home directory is known"
            )
        }
        Connected::On {
            endpoint,
            refused: false,
        } => format!("connected mode: on, sending to {endpoint}"),
        Connected::On {
            endpoint,
            refused: true,
        } => format!(
            "connected mode: on, sending to {endpoint}, which is refused: only an https address, or an http one on the loopback interface, is contacted"
        ),
    }
}

fn waiting_line(connected: &Connected, waiting: &Waiting) -> String {
    let mut parts = Vec::new();
    if let Connected::On { .. } = connected {
        if waiting.for_address > 0 || waiting.other_addresses > 0 {
            let mut ours = format!("{} for this address", waiting.for_address);
            if waiting.later > 0 {
                ours = format!("{ours}, {} of them later", waiting.later);
            }
            parts.push(ours);
        }
        if waiting.other_addresses > 0 {
            parts.push(format!("{} for other addresses", waiting.other_addresses));
        }
    } else if !waiting.by_address.is_empty() {
        let each: Vec<String> = waiting
            .by_address
            .iter()
            .map(|(address, count)| format!("{count} for {address}"))
            .collect();
        parts.push(each.join(", "));
    }
    if waiting.skipped > 0 {
        parts.push(format!("{} of a newer loft", waiting.skipped));
    }
    if parts.is_empty() {
        "waiting: none".to_owned()
    } else {
        format!("waiting: {}", parts.join("; "))
    }
}

fn last_line(last: Option<&Record>, now: u64) -> String {
    let Some(last) = last else {
        return "last attempt: none".to_owned();
    };
    let mut line = format!("last attempt: {} {}", time(last.ended), last.outcome);
    if let Some(why) = &last.why {
        line = format!("{line}: {why}");
    }
    if let Some(after) = last.paused(now) {
        line = format!(
            "{line}; Yard Server asked to come back after {}",
            time(after)
        );
    }
    line
}

fn fields(report: &Report) -> Value {
    let mut fields = Map::new();
    let (connected, missing, refused) = match &report.connected {
        Connected::Off(missing) => (false, missing.clone(), false),
        Connected::Homeless => (false, Vec::new(), false),
        Connected::On { endpoint, refused } => {
            fields.insert("endpoint".to_owned(), json!(endpoint));
            (true, Vec::new(), *refused)
        }
    };
    fields.insert("connected".to_owned(), json!(connected));
    fields.insert("missing".to_owned(), json!(missing));
    fields.insert("refused".to_owned(), json!(refused));
    fields.insert("homeKnown".to_owned(), json!(report.home_known));
    let waiting = &report.waiting;
    let by_address: Vec<Value> = waiting
        .by_address
        .iter()
        .map(|(address, count)| json!({ "address": address, "waiting": count }))
        .collect();
    fields.insert(
        "waiting".to_owned(),
        json!({
            "forAddress": waiting.for_address,
            "otherAddresses": waiting.other_addresses,
            "later": waiting.later,
            "skipped": waiting.skipped,
            "byAddress": by_address,
        }),
    );
    fields.insert(
        "lastAttempt".to_owned(),
        report.last.as_ref().map_or(Value::Null, |last| {
            json!({
                "ended": time(last.ended),
                "outcome": last.outcome,
                "why": last.why,
                "notBefore": last.not_before.map(time),
            })
        }),
    );
    let dead: Vec<Value> = report
        .dead
        .iter()
        .map(|(file, reason)| json!({ "file": file, "reason": reason }))
        .collect();
    fields.insert("dead".to_owned(), json!(dead));
    Value::Object(fields)
}
