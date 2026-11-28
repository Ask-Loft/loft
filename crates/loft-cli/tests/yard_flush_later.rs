//! How often a flush sends again the versions Yard Server will take later: within an hour of its
//! last answer of `later` for the address, a run passes over the entries it put off and sends the
//! others; once the hour has passed, it sends them again with the rest.

use serde_json::{Value, json};
use software_plant::YardServer;

use crate::common::signed_in::real_now;
use crate::yard::Yard;
use crate::yard_connected::{
    STAGE_BATCH, connected, flush, flush_record, keys, later_after, notes, readdress, records_sent,
    seconds_of, write,
};

/// The version keys each request to `server` carried, in the order sent.
fn sent(server: &YardServer) -> Vec<Vec<String>> {
    records_sent(server)
        .iter()
        .map(|records| {
            let key = |record: &Value| record["version"]["key"].as_str().unwrap().to_owned();
            records.iter().map(key).collect()
        })
        .collect()
}

/// A Yard Log with one entry that `server` puts off until later, flushed once: the server and the
/// entry's key.
fn put_off(yard: &Yard) -> (YardServer, String) {
    write(yard, STAGE_BATCH, &connected("http://127.0.0.1:1"));
    let key = keys(yard)[0].clone();
    let server = YardServer::putting_off(&[(&key, "unknown_report_version")]);
    readdress(yard, &server.url());
    let (_, flushed) = flush(yard, &connected(&server.url()));
    assert_eq!(flushed["later"], 1);
    (server, key)
}

#[test]
fn a_run_with_an_answer_of_later_keeps_the_versions_put_off_away_for_an_hour() {
    let yard = Yard::new();
    let before = real_now();
    let (server, _) = put_off(&yard);
    let after = real_now();
    let until = seconds_of(&flush_record(&yard, &server.url())["laterNotBefore"]);
    assert!((before + 3600..=after + 3600).contains(&until), "{until}");
}

#[test]
fn within_the_hour_a_run_passes_over_the_entries_put_off_and_sends_the_others() {
    let yard = Yard::new();
    let (server, key) = put_off(&yard);
    let variables = connected(&server.url());
    let until = flush_record(&yard, &server.url())["laterNotBefore"].clone();
    write(&yard, &notes(1), &variables);
    let fresh: Vec<String> = keys(&yard)
        .into_iter()
        .filter(|each| *each != key)
        .collect();
    let (code, flushed) = flush(&yard, &variables);
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("sent")));
    assert_eq!(sent(&server)[1], fresh);
    assert_eq!(
        (&flushed["sent"], &flushed["later"]),
        (&json!(1), &json!(1))
    );
    assert_eq!(keys(&yard), [key]);
    assert_eq!(flush_record(&yard, &server.url())["laterNotBefore"], until);
}

#[test]
fn a_run_with_only_entries_put_off_within_the_hour_makes_no_request_and_is_empty() {
    let yard = Yard::new();
    let (server, _) = put_off(&yard);
    let (code, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("empty")));
    assert_eq!(server.requests().len(), 1);
    assert_eq!(
        (&flushed["later"], &flushed["remaining"]),
        (&json!(1), &json!(0))
    );
}

#[test]
fn once_the_hour_has_passed_a_run_sends_the_entries_put_off_again() {
    let yard = Yard::new();
    let (server, key) = put_off(&yard);
    later_after(&yard, &server.url(), real_now() - 1);
    flush(&yard, &connected(&server.url()));
    assert_eq!(sent(&server), [vec![key.clone()], vec![key]]);
}
