//! What a flush does with each entry once Yard Server has answered its batch: a version Yard
//! Server holds leaves the outbox, a version it will never take is moved into `dead/` with the
//! reason, a version it will take later stays, and an answer the flush cannot read settles
//! nothing. A `400` or `413` without Yard Server's own code settles nothing either, and a `413`
//! splits a batch of several versions in halves.

use std::fs;

use serde_json::{Value, json};
use software_plant::YardServer;

use crate::common::signed_in::real_now;
use crate::yard::Yard;
use crate::yard_connected::{
    STAGE_BATCH, connected, entries, flush, keys, later_after, notes, outbox, readdress,
    records_sent, write,
};

/// The files of `dead/`, each with the reason kept beside it.
fn dead(yard: &Yard) -> Vec<(String, String)> {
    let dir = outbox(yard).join("dead");
    let Ok(listing) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, String)> = listing
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| !name.ends_with(".reason"))
        .map(|name| {
            let reason = fs::read_to_string(dir.join(format!("{name}.reason"))).unwrap();
            (name, reason.trim_end().to_owned())
        })
        .collect();
    found.sort();
    found
}

/// Writes the stage offline and `count` notes for `server`'s address: the keys of the notes'
/// entries.
fn waiting(yard: &Yard, endpoint: &str, count: usize) -> Vec<String> {
    write(yard, STAGE_BATCH, &[]);
    write(yard, &notes(count), &connected(endpoint));
    keys(yard)
}

#[test]
fn an_entry_whose_version_is_stored_or_held_leaves_the_outbox_and_counts_as_sent() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    let key = keys(&yard)[0].clone();
    let file = outbox(&yard).join(format!("{key}.json"));
    let copy = fs::read(&file).unwrap();
    let (_, stored) = flush(&yard, &variables);
    assert_eq!(
        (&stored["outcome"], &stored["sent"]),
        (&json!("sent"), &json!(1))
    );
    fs::write(&file, copy).unwrap();
    let (code, held) = flush(&yard, &variables);
    assert_eq!((code, &held["outcome"]), (Some(0), &json!("sent")));
    assert_eq!(held["sent"], 1);
    assert_eq!(entries(&yard), Vec::<Value>::new());
}

#[test]
fn an_entry_whose_version_is_in_conflict_is_moved_into_dead_with_the_reason_conflict() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    let key = keys(&yard)[0].clone();
    let file = outbox(&yard).join(format!("{key}.json"));
    let mut other: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    other["record"]["version"]["title"] = json!("Another title");
    flush(&yard, &variables);
    fs::write(&file, other.to_string()).unwrap();
    let (code, flushed) = flush(&yard, &variables);
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("sent")));
    assert_eq!((&flushed["sent"], &flushed["dead"]), (&json!(0), &json!(1)));
    assert_eq!(
        dead(&yard),
        [(format!("{key}.json"), "conflict".to_owned())]
    );
    assert_eq!(keys(&yard), Vec::<String>::new());
}

#[test]
fn an_entry_whose_version_is_refused_is_moved_into_dead_with_the_code() {
    let yard = Yard::new();
    let keys_before = waiting(&yard, "http://127.0.0.1:1", 2);
    let server = YardServer::refusing_versions(&[(&keys_before[0], "too_old")]);
    // The entries name the address they were written for; rewrite them for the server's.
    let variables = connected(&server.url());
    readdress(&yard, &server.url());
    let (code, flushed) = flush(&yard, &variables);
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("sent")));
    assert_eq!((&flushed["sent"], &flushed["dead"]), (&json!(1), &json!(1)));
    assert_eq!(
        dead(&yard),
        [(
            format!("{}.json", keys_before[0]),
            "refused too_old".to_owned()
        )]
    );
}

#[test]
fn an_entry_whose_version_yard_server_will_take_later_stays_and_a_later_run_sends_it_again() {
    let yard = Yard::new();
    let put_off = waiting(&yard, "http://127.0.0.1:1", 3);
    let server = YardServer::putting_off(&[(&put_off[1], "unknown_report_version")]);
    let variables = connected(&server.url());
    readdress(&yard, &server.url());
    let (code, flushed) = flush(&yard, &variables);
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("sent")));
    assert_eq!(
        (&flushed["sent"], &flushed["later"], &flushed["remaining"]),
        (&json!(2), &json!(1), &json!(0))
    );
    assert_eq!(keys(&yard), [put_off[1].clone()]);
    assert_eq!(server.requests().len(), 1);
    later_after(&yard, &server.url(), real_now() - 1);
    flush(&yard, &variables);
    assert_eq!(server.requests().len(), 2);
    assert_eq!(keys(&yard), [put_off[1].clone()]);
}

#[test]
fn an_entry_a_run_has_taken_once_is_not_taken_again_in_the_same_run() {
    let yard = Yard::new();
    let put_off = waiting(&yard, "http://127.0.0.1:1", 17);
    let server = YardServer::putting_off(&[(&put_off[0], "unknown_report_version")]);
    readdress(&yard, &server.url());
    let (_, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!(flushed["later"], 1);
    assert_eq!(batch_sizes(&server), [16, 1]);
}

#[test]
fn after_an_answer_of_200_that_is_not_json_every_entry_stays_and_the_flush_fails() {
    let yard = Yard::new();
    let server = YardServer::answering("not json");
    waiting(&yard, &server.url(), 2);
    let (code, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!((code, &flushed["outcome"]), (Some(1), &json!("failed")));
    assert_eq!(keys(&yard).len(), 2);
    assert_eq!(flushed["errors"][0]["kind"], "yard-server");
}

#[test]
fn an_entry_the_answer_leaves_out_or_gives_an_unknown_outcome_stays_and_the_rest_are_settled() {
    let yard = Yard::new();
    let put = waiting(&yard, "http://127.0.0.1:1", 3);
    let body = json!({
        "format": "yardlog-answer/1",
        "outcomes": [
            {"version": put[0], "outcome": "stored"},
            {"version": put[1], "outcome": "perhaps"},
        ],
    });
    let server = YardServer::answering(&body.to_string());
    readdress(&yard, &server.url());
    let (code, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!((code, &flushed["outcome"]), (Some(1), &json!("failed")));
    assert_eq!(keys(&yard), [put[1].clone(), put[2].clone()]);
    assert_eq!(flushed["sent"], 1);
}

#[test]
fn after_a_400_every_entry_of_the_batch_is_in_dead_and_the_next_batch_is_sent() {
    let yard = Yard::new();
    let server = YardServer::failing(400);
    let put = waiting(&yard, &server.url(), 17);
    let (code, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("sent")));
    assert_eq!(server.requests().len(), 2);
    assert_eq!(flushed["dead"], 17);
    let reasons = dead(&yard);
    assert_eq!(reasons.len(), 17);
    assert_eq!(
        reasons[0],
        (format!("{}.json", put[0]), "400 invalid_request".to_owned())
    );
}

#[test]
fn after_a_413_the_batch_s_one_entry_is_in_dead_with_the_status_and_code() {
    let yard = Yard::new();
    let server = YardServer::failing(413);
    let put = waiting(&yard, &server.url(), 1);
    let (_, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!(flushed["dead"], 1);
    assert_eq!(
        dead(&yard),
        [(
            format!("{}.json", put[0]),
            "413 payload_too_large".to_owned()
        )]
    );
}

/// The number of records each request to `server` carried, in the order sent.
fn batch_sizes(server: &YardServer) -> Vec<usize> {
    records_sent(server).iter().map(Vec::len).collect()
}

/// The sizes of the entry files of `keys`.
fn sizes(yard: &Yard, keys: &[String]) -> Vec<usize> {
    let dir = outbox(yard);
    keys.iter()
        .map(|key| {
            let length = fs::metadata(dir.join(format!("{key}.json"))).unwrap().len();
            usize::try_from(length).unwrap()
        })
        .collect()
}

#[test]
fn after_a_400_that_is_not_yard_server_s_own_every_entry_stays_and_the_flush_fails() {
    for body in [
        "<html><body>400 Bad Request</body></html>",
        "",
        r#"{"title":"Bad Request","code":"bad_gateway_rule"}"#,
        r#"{"title":"Bad Request"}"#,
    ] {
        let yard = Yard::new();
        let server = YardServer::answering_with(400, body);
        let put = waiting(&yard, &server.url(), 2);
        let (code, flushed) = flush(&yard, &connected(&server.url()));
        assert_eq!((code, &flushed["outcome"]), (Some(1), &json!("failed")));
        assert_eq!(flushed["dead"], 0, "{body}");
        assert_eq!(dead(&yard), Vec::new());
        assert_eq!(keys(&yard), put);
        assert_eq!(flushed["errors"][0]["kind"], "yard-server");
        let message = flushed["errors"][0]["message"].as_str().unwrap();
        assert!(message.contains("400"), "{message}");
        assert!(message.contains("not Yard Server"), "{message}");
    }
}

#[test]
fn after_a_413_without_its_code_to_one_entry_the_entry_stays_and_the_flush_fails() {
    let yard = Yard::new();
    let server = YardServer::answering_with(413, "Request Entity Too Large");
    let put = waiting(&yard, &server.url(), 1);
    let (code, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!((code, &flushed["outcome"]), (Some(1), &json!("failed")));
    assert_eq!(dead(&yard), Vec::new());
    assert_eq!(keys(&yard), put);
    let message = flushed["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("413"), "{message}");
}

#[test]
fn a_413_without_its_code_splits_a_batch_of_several_versions_as_well() {
    let yard = Yard::new();
    let server = YardServer::answering_with(413, "Request Entity Too Large");
    let put = waiting(&yard, &server.url(), 2);
    let (_, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!(batch_sizes(&server), [2, 1]);
    assert_eq!(flushed["outcome"], "failed");
    assert_eq!(keys(&yard), put);
}

#[test]
fn after_a_413_to_a_batch_of_4_the_halves_go_older_first_and_none_is_moved_into_dead() {
    let yard = Yard::new();
    let probe = YardServer::accepting();
    let put = waiting(&yard, &probe.url(), 4);
    readdress(&yard, &probe.url());
    let sizes = sizes(&yard, &put);
    let pair = (sizes[0] + sizes[1]).max(sizes[2] + sizes[3]);
    let server = YardServer::limiting(pair + 64);
    readdress(&yard, &server.url());
    let (code, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("sent")));
    assert_eq!(batch_sizes(&server), [4, 2, 2]);
    let held: Vec<Value> = server
        .records()
        .iter()
        .map(|record| record["version"]["key"].clone())
        .collect();
    assert_eq!(held, put.iter().map(|key| json!(key)).collect::<Vec<_>>());
    assert_eq!((&flushed["sent"], &flushed["dead"]), (&json!(4), &json!(0)));
    assert_eq!(dead(&yard), Vec::new());
    assert_eq!(keys(&yard), Vec::<String>::new());
}

#[test]
fn a_half_answered_413_again_is_split_again_and_each_entry_goes_once() {
    let yard = Yard::new();
    let probe = YardServer::accepting();
    let put = waiting(&yard, &probe.url(), 4);
    readdress(&yard, &probe.url());
    let largest = sizes(&yard, &put).into_iter().max().unwrap();
    let server = YardServer::limiting(largest + 64);
    readdress(&yard, &server.url());
    let (_, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!(batch_sizes(&server), [4, 2, 1, 1, 2, 1, 1]);
    assert_eq!((&flushed["sent"], &flushed["dead"]), (&json!(4), &json!(0)));
    assert_eq!(server.records().len(), 4);
}

#[cfg(unix)]
#[test]
fn an_entry_that_cannot_be_removed_once_answered_stays_and_the_flush_fails_naming_it() {
    use std::os::unix::fs::PermissionsExt as _;
    let yard = Yard::new();
    let server = YardServer::accepting();
    let put = waiting(&yard, &server.url(), 1);
    let dir = outbox(&yard);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    let (code, flushed) = flush(&yard, &connected(&server.url()));
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!((code, &flushed["outcome"]), (Some(1), &json!("failed")));
    assert_eq!(flushed["errors"][0]["kind"], "outbox");
    let message = flushed["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains(&put[0]), "{message}");
    assert_eq!(server.requests().len(), 1);
    assert_eq!(keys(&yard), put);
}
