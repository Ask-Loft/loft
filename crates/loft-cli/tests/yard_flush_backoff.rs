//! How a flush keeps away from Yard Server after batches that did not go: a pause of 5 seconds
//! after the first run in a row that made a request and failed, doubled after each further one up
//! to 5 minutes and drawn at random between half of it and all of it, recorded in
//! `~/.yard/flush.json` and given as `notBefore`, and cleared once a batch goes.

use std::fs;

use serde_json::{Value, json};
use software_plant::YardServer;

use crate::common::signed_in::real_now;
use crate::yard::Yard;
use crate::yard_connected::{
    STAGE_BATCH, come_back_after, connected, flush, flush_record, seconds_of, set_record_field,
    write,
};

/// How many seconds after the run ended the record of `endpoint` keeps the flush away.
fn pause_of(yard: &Yard, endpoint: &str) -> u64 {
    let record = flush_record(yard, endpoint);
    seconds_of(&record["notBefore"]) - seconds_of(&record["ended"])
}

/// Sets the runs in a row that failed in the record of `endpoint` to `failures`, and ends its
/// pause.
fn failed_before(yard: &Yard, endpoint: &str, failures: u64) {
    come_back_after(yard, endpoint, real_now() - 1);
    set_record_field(yard, endpoint, "failures", json!(failures));
}

#[test]
fn after_the_first_failed_run_the_flush_keeps_away_from_2_5_to_5_seconds() {
    let yard = Yard::new();
    let server = YardServer::failing(500);
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    let (code, failed) = flush(&yard, &variables);
    assert_eq!((code, &failed["outcome"]), (Some(1), &json!("failed")));
    assert_eq!(flush_record(&yard, &server.url())["failures"], 1);
    assert!((3..=5).contains(&pause_of(&yard, &server.url())));
    assert_eq!(
        failed["notBefore"],
        flush_record(&yard, &server.url())["notBefore"]
    );
}

#[test]
fn a_run_within_the_pause_makes_no_request_and_one_after_it_sends_again() {
    let yard = Yard::new();
    let server = YardServer::failing(503);
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    flush(&yard, &variables);
    let (code, paused) = flush(&yard, &variables);
    assert_eq!((code, &paused["outcome"]), (Some(0), &json!("paused")));
    assert_eq!(server.requests().len(), 1);
    assert!(paused["notBefore"].is_string());
    come_back_after(&yard, &server.url(), real_now() - 1);
    flush(&yard, &variables);
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn each_further_failed_run_in_a_row_doubles_the_pause_up_to_5_minutes() {
    let yard = Yard::new();
    let server = YardServer::failing(401);
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    flush(&yard, &variables);
    failed_before(&yard, &server.url(), 1);
    flush(&yard, &variables);
    assert_eq!(flush_record(&yard, &server.url())["failures"], 2);
    assert!((5..=10).contains(&pause_of(&yard, &server.url())));
    for failures in [6, 7, 30] {
        failed_before(&yard, &server.url(), failures);
        flush(&yard, &variables);
        assert_eq!(flush_record(&yard, &server.url())["failures"], failures + 1);
        let pause = pause_of(&yard, &server.url());
        assert!((150..=300).contains(&pause), "{failures}: {pause}");
    }
}

#[test]
fn a_batch_that_goes_sets_the_failures_in_a_row_back_to_none() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    flush(&yard, &variables);
    failed_before(&yard, &server.url(), 4);
    write(&yard, STAGE_BATCH, &variables);
    let (code, sent) = flush(&yard, &variables);
    assert_eq!((code, &sent["outcome"]), (Some(0), &json!("sent")));
    let record = flush_record(&yard, &server.url());
    assert_eq!(record["failures"], 0);
    assert_eq!(record["notBefore"], Value::Null);
    assert_eq!(sent["notBefore"], Value::Null);
}

#[test]
fn a_retry_after_leaves_the_failures_in_a_row_as_they_were() {
    let yard = Yard::new();
    let server = YardServer::coming_back(429, "120");
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    flush(&yard, &variables);
    failed_before(&yard, &server.url(), 2);
    let (_, paused) = flush(&yard, &variables);
    assert_eq!(paused["outcome"], "paused");
    assert_eq!(flush_record(&yard, &server.url())["failures"], 2);
    assert_eq!(pause_of(&yard, &server.url()), 120);
}

#[test]
fn a_run_that_fails_without_a_request_adds_no_pause() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let refused = vec![
        ("LOFT_YARD_SERVER_ENDPOINT", "http://example.com".to_owned()),
        ("LOFT_YARD_SERVER_TOKEN", "token".to_owned()),
    ];
    let mut timing_out = connected(&server.url()).to_vec();
    timing_out.push(("LOFT_YARD_SERVER_TIMEOUT", "61".to_owned()));
    for variables in [refused, timing_out] {
        write(&yard, STAGE_BATCH, &variables);
        let (code, failed) = flush(&yard, &variables);
        assert_eq!((code, &failed["outcome"]), (Some(1), &json!("failed")));
        assert_eq!(failed["notBefore"], Value::Null);
        let endpoint = &variables[0].1;
        assert_eq!(flush_record(&yard, endpoint)["notBefore"], Value::Null);
        assert_eq!(flush_record(&yard, endpoint)["failures"], 0);
    }
    assert_eq!(server.requests().len(), 0);
}

#[cfg(unix)]
#[test]
fn a_run_that_fails_only_on_the_outbox_adds_no_pause() {
    use std::os::unix::fs::PermissionsExt as _;

    use crate::yard_connected::{keys, outbox};
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    let dir = outbox(&yard);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    let (code, failed) = flush(&yard, &variables);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!((code, &failed["outcome"]), (Some(1), &json!("failed")));
    assert_eq!(failed["errors"][0]["kind"], "outbox");
    assert_eq!(flush_record(&yard, &server.url())["notBefore"], Value::Null);
    assert_eq!(keys(&yard).len(), 1);
}

#[test]
fn a_pause_that_cannot_be_recorded_is_still_given_as_not_before() {
    let yard = Yard::new();
    let server = YardServer::failing(500);
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    fs::create_dir_all(crate::yard_connected::flush_json(&yard)).unwrap();
    let (code, failed) = flush(&yard, &variables);
    assert_eq!((code, &failed["outcome"]), (Some(1), &json!("failed")));
    let not_before = seconds_of(&failed["notBefore"]);
    let now = real_now();
    assert!((now..=now + 5).contains(&not_before), "{not_before}");
}
