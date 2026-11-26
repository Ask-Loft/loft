use std::cell::Cell;
use std::fs;
use std::time::Duration;

use loft_ports::PortError;
use loft_ports::clock::Clock;
use software_plant::YardServer;
use yardlog_store::outbox::Outbox;

use super::sending::{MOST_TIME, drain};

const WAIT: Duration = Duration::from_secs(2);

/// A clock whose every reading of the elapsed time finds `step` more passed than the last.
struct Stepping {
    step: Duration,
    passed: Cell<Duration>,
}

impl Clock for Stepping {
    fn now(&self) -> Result<u64, PortError> {
        Ok(0)
    }

    fn elapsed(&self) -> Duration {
        let passed = self.passed.get();
        self.passed.set(passed + self.step);
        passed
    }

    fn wait(&self, _length: Duration) {}
}

/// An outbox for `endpoint` in a fresh directory, holding `count` small entries.
fn filled(endpoint: &str, count: usize) -> Outbox {
    let dir = std::env::temp_dir().join(format!("loft-flush-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    for n in 0..count {
        let key = format!("00000000-0000-7000-8000-{n:012}");
        let record = format!(
            r#"{{"record":"version","yardLog":"y","kind":"note","id":"n{n}","version":{{"key":"{key}"}}}}"#
        );
        fs::write(
            dir.join(format!("{key}.json")),
            yardlog::entry::write(endpoint, &record),
        )
        .unwrap();
    }
    Outbox::new(dir, endpoint)
}

#[test]
fn a_run_that_has_sent_for_5_minutes_ends_after_the_batch_on_its_way_and_counts_the_rest() {
    let server = YardServer::accepting();
    let outbox = filled(&server.url(), 40);
    // The run reads the clock as it begins and before each batch: at 0, 2 and 4 minutes it takes
    // a batch, and at 6 minutes, past the 5, it takes no more.
    let clock = Stepping {
        step: MOST_TIME / 5 * 2,
        passed: Cell::new(Duration::ZERO),
    };
    let flushed = drain(&outbox, software_plant::yard_server::TOKEN, WAIT, &clock);
    assert_eq!(server.requests().len(), 2);
    assert_eq!(flushed.outcome, "sent");
    assert_eq!((flushed.sent, flushed.remaining()), (32, 8));
    fs::remove_dir_all(outbox.dir()).unwrap();
}

#[test]
fn a_run_with_time_to_spare_sends_every_batch() {
    let server = YardServer::accepting();
    let outbox = filled(&server.url(), 40);
    let clock = Stepping {
        step: Duration::from_secs(1),
        passed: Cell::new(Duration::ZERO),
    };
    let flushed = drain(&outbox, software_plant::yard_server::TOKEN, WAIT, &clock);
    assert_eq!(server.requests().len(), 3);
    assert_eq!((flushed.sent, flushed.remaining()), (40, 0));
    fs::remove_dir_all(outbox.dir()).unwrap();
}

#[test]
fn a_flush_waits_10_seconds_for_an_answer_unless_the_timeout_says_otherwise() {
    assert_eq!(super::wait_of(None), Duration::from_secs(10));
    assert_eq!(super::wait_of(Some(3)), Duration::from_secs(3));
}

/// Yard Server that stores the first batch and asks to come back later to every batch after it.
struct StoresThenComesBack {
    answered: usize,
}

impl software_plant::http::Responder for StoresThenComesBack {
    fn answer(&mut self, request: &software_plant::http::Request) -> software_plant::http::Answer {
        self.answered += 1;
        if self.answered > 1 {
            let mut response = software_plant::http::Response::empty(429);
            response
                .headers
                .push(("Retry-After".to_owned(), "60".to_owned()));
            return response.into();
        }
        let batch: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        let outcomes: Vec<serde_json::Value> = batch["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|record| serde_json::json!({"version": record["version"]["key"], "outcome": "stored"}))
            .collect();
        software_plant::http::Response::json(
            200,
            &serde_json::json!({"format": "yardlog-answer/1", "outcomes": outcomes}),
        )
        .into()
    }
}

#[test]
fn a_run_asked_to_come_back_later_ends_paused_even_where_earlier_batches_went() {
    let server = software_plant::http::Server::start(StoresThenComesBack { answered: 0 });
    let outbox = filled(&server.url(), 20);
    let clock = Stepping {
        step: Duration::from_secs(1),
        passed: Cell::new(Duration::ZERO),
    };
    let flushed = drain(&outbox, "token", WAIT, &clock);
    assert_eq!(flushed.outcome, "paused");
    assert_eq!(flushed.sent, 16);
    assert_eq!(flushed.not_before, Some(60));
    assert_eq!(flushed.remaining(), 4);
    fs::remove_dir_all(outbox.dir()).unwrap();
}

/// A clock whose time, in seconds since the Unix epoch, is what its cell holds.
struct Shared(std::sync::Arc<std::sync::atomic::AtomicU64>);

impl Clock for Shared {
    fn now(&self) -> Result<u64, PortError> {
        Ok(self.0.load(std::sync::atomic::Ordering::SeqCst))
    }

    fn elapsed(&self) -> Duration {
        Duration::ZERO
    }

    fn wait(&self, _length: Duration) {}
}

/// Yard Server that takes 9 seconds of the shared clock to ask to come back 5 seconds later.
struct SlowToComeBack(std::sync::Arc<std::sync::atomic::AtomicU64>);

impl software_plant::http::Responder for SlowToComeBack {
    fn answer(&mut self, _request: &software_plant::http::Request) -> software_plant::http::Answer {
        self.0.fetch_add(9, std::sync::atomic::Ordering::SeqCst);
        let mut response = software_plant::http::Response::empty(429);
        response
            .headers
            .push(("Retry-After".to_owned(), "5".to_owned()));
        response.into()
    }
}

#[test]
fn a_retry_after_in_seconds_counts_from_the_answer() {
    let time = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1000));
    let server = software_plant::http::Server::start(SlowToComeBack(time.clone()));
    let outbox = filled(&server.url(), 1);
    let flushed = drain(&outbox, "token", WAIT, &Shared(time));
    assert_eq!(flushed.not_before, Some(1014));
    fs::remove_dir_all(outbox.dir()).unwrap();
}

fn keys(keys: &[&str]) -> std::collections::BTreeSet<String> {
    keys.iter().map(|key| (*key).to_owned()).collect()
}

fn codes(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, code)| ((*key).to_owned(), (*code).to_owned()))
        .collect()
}

#[test]
fn a_record_that_replaces_another_keeps_a_time_to_come_back_after_that_has_not_passed() {
    let previous = super::record::Record {
        not_before: Some(500),
        ..super::record::Record::default()
    };
    let (taken, later) = (keys(&[]), codes(&[]));
    let run = |ended| super::record::Run {
        ended,
        outcome: "failed",
        why: None,
        not_before: None,
        taken: &taken,
        later: &later,
    };
    assert_eq!(
        super::record::next(&previous, &run(400), &keys(&[])).not_before,
        Some(500)
    );
    assert_eq!(
        super::record::next(&previous, &run(500), &keys(&[])).not_before,
        None
    );
}

#[test]
fn a_record_keeps_the_later_key_of_each_entry_the_run_did_not_take_while_it_waits() {
    let previous = super::record::Record {
        later: codes(&[("a", "old"), ("b", "old"), ("c", "old")]),
        ..super::record::Record::default()
    };
    let (taken, later) = (keys(&["b"]), codes(&[("d", "new")]));
    let run = super::record::Run {
        ended: 1,
        outcome: "sent",
        why: None,
        not_before: None,
        taken: &taken,
        later: &later,
    };
    // `a` was not taken and waits; `b` was taken and not put off again; `c` is gone.
    let next = super::record::next(&previous, &run, &keys(&["a", "b", "d"]));
    assert_eq!(next.later, codes(&[("a", "old"), ("d", "new")]));
}

/// Connected mode on for `endpoint`, with the home of Yard in a fresh directory.
fn on(endpoint: &str) -> super::Mode {
    let dir = std::env::temp_dir().join(format!("loft-due-{}", uuid::Uuid::new_v4()));
    let home = yardlog_store::home::Home::at(&dir);
    let outbox = home.outbox(endpoint);
    super::Mode::On(super::Connection {
        home,
        outbox,
        token: "token".to_owned(),
    })
}

/// Puts an entry of `format` for `endpoint` into the outbox of `mode`, named by `key`.
fn put(mode: &super::Mode, endpoint: &str, key: &str, format: &str) {
    let super::Mode::On(connection) = mode else {
        unreachable!()
    };
    let dir = connection.outbox.dir();
    fs::create_dir_all(dir).unwrap();
    let record = format!(r#"{{"record":"version","version":{{"key":"{key}"}}}}"#);
    let mut entry: serde_json::Value =
        serde_json::from_str(&yardlog::entry::write(endpoint, &record)).unwrap();
    entry["format"] = serde_json::json!(format);
    fs::write(dir.join(format!("{key}.json")), entry.to_string()).unwrap();
}

/// Writes `record` as the last run for `endpoint` in the home of `mode`.
fn recorded(mode: &super::Mode, endpoint: &str, record: &serde_json::Value) {
    let super::Mode::On(connection) = mode else {
        unreachable!()
    };
    fs::create_dir_all(connection.home.dir()).unwrap();
    fs::write(
        connection.home.dir().join("flush.json"),
        serde_json::json!({ endpoint: record }).to_string(),
    )
    .unwrap();
}

const ADDRESS: &str = "http://127.0.0.1:1";

#[test]
fn a_flush_is_due_once_an_entry_for_the_address_waits() {
    let mode = on(ADDRESS);
    assert!(!super::due(&mode, 0));
    put(
        &mode,
        ADDRESS,
        "00000000-0000-7000-8000-000000000001",
        "yardlog-outbox/1",
    );
    assert!(super::due(&mode, 0));
}

#[test]
fn no_flush_is_due_without_connected_mode() {
    assert!(!super::due(&super::Mode::Off(Vec::new()), 0));
    assert!(!super::due(&super::Mode::Homeless, 0));
}

#[test]
fn no_flush_is_due_for_entries_of_another_address_or_of_a_newer_format() {
    let mode = on(ADDRESS);
    put(
        &mode,
        "http://127.0.0.1:2",
        "00000000-0000-7000-8000-000000000001",
        "yardlog-outbox/1",
    );
    put(
        &mode,
        ADDRESS,
        "00000000-0000-7000-8000-000000000002",
        "yardlog-outbox/9",
    );
    assert!(!super::due(&mode, 0));
}

#[test]
fn no_flush_is_due_for_entries_yard_server_will_take_later_alone() {
    let mode = on(ADDRESS);
    let key = "00000000-0000-7000-8000-000000000001";
    put(&mode, ADDRESS, key, "yardlog-outbox/1");
    recorded(
        &mode,
        ADDRESS,
        &serde_json::json!({"ended": "2026-01-01T00:00:00Z", "outcome": "sent", "later": {key: "unknown_report_version"}}),
    );
    assert!(!super::due(&mode, 0));
    put(
        &mode,
        ADDRESS,
        "00000000-0000-7000-8000-000000000002",
        "yardlog-outbox/1",
    );
    assert!(super::due(&mode, 0));
}

#[test]
fn no_flush_is_due_before_the_time_yard_server_asked_to_come_back_after() {
    let mode = on(ADDRESS);
    put(
        &mode,
        ADDRESS,
        "00000000-0000-7000-8000-000000000001",
        "yardlog-outbox/1",
    );
    recorded(
        &mode,
        ADDRESS,
        &serde_json::json!({"ended": "2026-01-01T00:00:00Z", "outcome": "paused", "notBefore": "2026-01-01T00:02:00Z"}),
    );
    let at = 1_767_225_600;
    assert!(!super::due(&mode, at + 119));
    assert!(super::due(&mode, at + 120));
}
