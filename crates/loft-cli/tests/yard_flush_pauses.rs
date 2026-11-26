//! When a flush keeps away from Yard Server, and what it records of each run: a `Retry-After`
//! keeps it away until the time it names, `LOFT_YARD_SERVER_TIMEOUT` sets how long it waits for
//! an answer, and `~/.yard/flush.json` records each address's last run.

use std::fs;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use software_plant::yard_server::TOKEN;
use software_plant::{Clock, YardServer};
use yardlog_store::lock;

use crate::common::EPOCH;
use crate::yard::{Yard, document};
use crate::yard_connected::{
    STAGE_BATCH, connected, entries, flush, flush_json, keys, notes, outbox, put_off_record,
    readdress, write,
};

/// The record `~/.yard/flush.json` holds for `endpoint`.
fn record(yard: &Yard, endpoint: &str) -> Value {
    let text = fs::read_to_string(flush_json(yard)).unwrap_or_default();
    serde_json::from_str::<Value>(&text).unwrap_or(Value::Null)[endpoint].clone()
}

/// `variables` with `LOFT_YARD_SERVER_TIMEOUT` set to `value`.
fn timing_out(variables: &[(&'static str, String)], value: &str) -> Vec<(&'static str, String)> {
    let mut all = variables.to_vec();
    all.push(("LOFT_YARD_SERVER_TIMEOUT", value.to_owned()));
    all
}

#[test]
fn a_429_with_a_retry_after_in_seconds_pauses_the_flush_until_the_time_it_names() {
    let mut yard = Yard::new();
    let server = YardServer::coming_back(429, "120");
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    let (code, paused) = flush(&yard, &variables);
    assert_eq!((code, &paused["outcome"]), (Some(0), &json!("paused")));
    assert_eq!(entries(&yard).len(), 1);
    assert_eq!(
        record(&yard, &server.url())["notBefore"],
        "2026-01-01T00:02:00Z"
    );
    let (code, again) = flush(&yard, &variables);
    assert_eq!((code, &again["outcome"]), (Some(0), &json!("paused")));
    assert_eq!(again["remaining"], 1);
    assert_eq!(server.requests().len(), 1);
    yard.clock = Clock::fixed(EPOCH + 121);
    flush(&yard, &variables);
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn a_503_with_a_retry_after_as_an_http_date_pauses_the_flush_until_that_date() {
    let mut yard = Yard::new();
    let server = YardServer::coming_back(503, "Thu, 01 Jan 2026 01:00:00 GMT");
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    let (_, paused) = flush(&yard, &variables);
    assert_eq!(paused["outcome"], "paused");
    yard.clock = Clock::fixed(EPOCH + 3599);
    flush(&yard, &variables);
    assert_eq!(server.requests().len(), 1);
    yard.clock = Clock::fixed(EPOCH + 3601);
    flush(&yard, &variables);
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn a_retry_after_of_0_seconds_or_of_a_date_already_past_lets_the_next_run_send() {
    for retry_after in ["0", "Wed, 31 Dec 2025 00:00:00 GMT"] {
        let yard = Yard::new();
        let server = YardServer::coming_back(429, retry_after);
        let variables = connected(&server.url());
        write(&yard, STAGE_BATCH, &variables);
        let (_, paused) = flush(&yard, &variables);
        assert_eq!(paused["outcome"], "paused", "{retry_after}");
        flush(&yard, &variables);
        assert_eq!(server.requests().len(), 2, "{retry_after}");
    }
}

#[test]
fn a_429_or_503_without_a_retry_after_that_reads_is_a_failure() {
    for server in [
        YardServer::failing(429),
        YardServer::failing(503),
        YardServer::coming_back(429, "soon"),
        YardServer::coming_back(503, "99999999999999"),
    ] {
        let yard = Yard::new();
        let variables = connected(&server.url());
        write(&yard, STAGE_BATCH, &variables);
        let (code, failed) = flush(&yard, &variables);
        assert_eq!((code, &failed["outcome"]), (Some(1), &json!("failed")));
        assert_eq!(record(&yard, &server.url())["notBefore"], Value::Null);
    }
}

#[test]
fn a_paused_run_checks_the_time_before_it_reads_the_outbox() {
    let yard = Yard::new();
    let server = YardServer::coming_back(429, "120");
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    flush(&yard, &variables);
    fs::write(
        outbox(&yard).join("00000000-0000-7000-8000-000000000000.json"),
        "{broken",
    )
    .unwrap();
    let (_, paused) = flush(&yard, &variables);
    assert_eq!(
        (&paused["outcome"], &paused["dead"]),
        (&json!("paused"), &json!(0))
    );
    assert!(!outbox(&yard).join("dead").exists());
}

#[test]
fn with_a_timeout_of_1_second_a_silent_yard_server_is_given_up_after_it() {
    let yard = Yard::new();
    let server = YardServer::silent();
    let variables = timing_out(&connected(&server.url()), "1");
    write(&yard, STAGE_BATCH, &variables);
    let started = Instant::now();
    let (code, failed) = flush(&yard, &variables);
    let took = started.elapsed();
    assert_eq!((code, &failed["outcome"]), (Some(1), &json!("failed")));
    assert!(took >= Duration::from_secs(1), "{took:?}");
    assert!(took < Duration::from_secs(4), "{took:?}");
    let message = failed["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("did not answer"), "{message}");
    assert_eq!(entries(&yard).len(), 1);
}

#[test]
fn a_timeout_that_is_not_a_positive_whole_number_of_seconds_fails_with_no_request() {
    for value in ["soon", "0", "-3", "1.5"] {
        let yard = Yard::new();
        let server = YardServer::accepting();
        let variables = timing_out(&connected(&server.url()), value);
        write(&yard, STAGE_BATCH, &variables);
        let (code, failed) = flush(&yard, &variables);
        assert_eq!(
            (code, &failed["outcome"]),
            (Some(1), &json!("failed")),
            "{value}"
        );
        let message = failed["errors"][0]["message"].as_str().unwrap();
        assert!(
            message.contains(&format!("LOFT_YARD_SERVER_TIMEOUT={value}")),
            "{message}"
        );
        assert_eq!(server.requests().len(), 0, "{value}");
        assert_eq!(failed["remaining"], 1, "{value}");
    }
}

#[test]
fn with_both_variables_set_and_no_home_directory_known_a_flush_fails_naming_it() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let output = yard
        .scratch
        .command(Some(&yard.git), &yard.clock)
        .envs(connected(&server.url()))
        .env("HOME", "not-an-absolute-home")
        .args(["yard", "flush", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let failed = document("flush", &output);
    assert_eq!(failed["outcome"], "failed");
    let message = failed["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("home directory"), "{message}");
    assert!(!message.contains("off"), "{message}");
}

#[test]
fn flush_json_holds_a_record_for_each_address_a_run_has_run_for() {
    let yard = Yard::new();
    let (ours, theirs) = (YardServer::accepting(), YardServer::failing(500));
    write(&yard, STAGE_BATCH, &connected(&ours.url()));
    write(&yard, &notes(1), &connected(&theirs.url()));
    flush(&yard, &connected(&ours.url()));
    flush(&yard, &connected(&theirs.url()));
    let sent = record(&yard, &ours.url());
    assert_eq!(
        (&sent["outcome"], &sent["ended"]),
        (&json!("sent"), &json!("2026-01-01T00:00:00Z"))
    );
    assert_eq!(sent["why"], Value::Null);
    let failed = record(&yard, &theirs.url());
    assert_eq!(failed["outcome"], "failed");
    assert!(failed["why"].as_str().unwrap().contains("500"), "{failed}");
    let idle = YardServer::accepting();
    flush(&yard, &connected(&idle.url()));
    assert_eq!(record(&yard, &idle.url())["outcome"], "empty");
    let text = fs::read_to_string(flush_json(&yard)).unwrap();
    assert!(!text.contains(TOKEN), "{text}");
}

#[test]
fn a_busy_run_or_one_paused_without_a_request_leaves_the_record_as_it_was() {
    let mut yard = Yard::new();
    let server = YardServer::coming_back(503, "120");
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    flush(&yard, &variables);
    let before = fs::read(flush_json(&yard)).unwrap();
    yard.clock = Clock::fixed(EPOCH + 60);
    let (_, paused) = flush(&yard, &variables);
    assert_eq!(paused["outcome"], "paused");
    assert_eq!(server.requests().len(), 1);
    assert_eq!(fs::read(flush_json(&yard)).unwrap(), before);
    let _held = lock::try_lock(&yard.scratch.home().join(".yard/flush.lock"))
        .unwrap()
        .unwrap();
    let (_, busy) = flush(&yard, &variables);
    assert_eq!(busy["outcome"], "busy");
    assert_eq!(fs::read(flush_json(&yard)).unwrap(), before);
}

#[test]
fn a_run_records_the_versions_yard_server_will_take_later_with_their_codes() {
    let yard = Yard::new();
    write(&yard, STAGE_BATCH, &connected("http://127.0.0.1:1"));
    let key = keys(&yard)[0].clone();
    let server = YardServer::putting_off(&[(&key, "unknown_report_version")]);
    readdress(&yard, &server.url());
    flush(&yard, &connected(&server.url()));
    assert_eq!(
        record(&yard, &server.url())["later"],
        json!({ key: "unknown_report_version" })
    );
}

#[test]
fn a_busy_or_paused_run_counts_as_later_the_entries_the_record_holds_as_later() {
    let yard = Yard::new();
    write(&yard, STAGE_BATCH, &connected("http://127.0.0.1:1"));
    let key = keys(&yard)[0].clone();
    let server = YardServer::putting_off(&[(&key, "unknown_report_version")]);
    readdress(&yard, &server.url());
    let variables = connected(&server.url());
    let (_, sent) = flush(&yard, &variables);
    let counts = |run: &Value| (run["later"].clone(), run["remaining"].clone());
    assert_eq!(counts(&sent), (json!(1), json!(0)));
    let mut all: Value = serde_json::from_slice(&fs::read(flush_json(&yard)).unwrap()).unwrap();
    all[server.url()]["notBefore"] = json!("2100-01-01T00:00:00Z");
    fs::write(flush_json(&yard), all.to_string()).unwrap();
    let (_, paused) = flush(&yard, &variables);
    assert_eq!(paused["outcome"], "paused");
    assert_eq!(counts(&paused), (json!(1), json!(0)));
    let _held = lock::try_lock(&yard.scratch.home().join(".yard/flush.lock"))
        .unwrap()
        .unwrap();
    let (_, busy) = flush(&yard, &variables);
    assert_eq!(busy["outcome"], "busy");
    assert_eq!(counts(&busy), (json!(1), json!(0)));
}

#[test]
fn a_flush_json_that_does_not_read_is_taken_as_holding_no_record_and_written_anew() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    fs::create_dir_all(flush_json(&yard).parent().unwrap()).unwrap();
    fs::write(flush_json(&yard), "{not json").unwrap();
    let (code, sent) = flush(&yard, &variables);
    assert_eq!((code, &sent["outcome"]), (Some(0), &json!("sent")));
    assert_eq!(record(&yard, &server.url())["outcome"], "sent");
}

#[test]
fn a_run_that_cannot_take_the_lock_counts_as_later_the_entries_the_record_holds_as_later() {
    use std::os::unix::fs::PermissionsExt as _;
    let yard = Yard::new();
    let variables = connected("http://127.0.0.1:1");
    write(&yard, STAGE_BATCH, &variables);
    let key = keys(&yard)[0].clone();
    let record = json!({ "http://127.0.0.1:1": put_off_record(&key) });
    fs::write(flush_json(&yard), record.to_string()).unwrap();
    let lock = yard.scratch.home().join(".yard/flush.lock");
    fs::write(&lock, "").unwrap();
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::OpenOptions::new().write(true).open(&lock).is_ok() {
        eprintln!("skipped: the lock file stays writable for this user, as for root");
        return;
    }
    let (_, failed) = flush(&yard, &variables);
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(failed["outcome"], "failed");
    assert_eq!(
        (&failed["later"], &failed["remaining"]),
        (&json!(1), &json!(0))
    );
}
