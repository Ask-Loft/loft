//! `loft yard upgrade` over the event logs of the Codex sessions whose totals it classified: each
//! zero cache-write count no verified API-key account reported moves under
//! `usageSource.reported`, so that replaying the log gives the totals the upgrade wrote, and each
//! line keeps its event identity, so that a retry of the original event is still recognized.

use std::fs;

use serde_json::{Value, json};

use crate::served::Served;
use crate::yard::{ASK, Yard};
use crate::yard_serve_events::{event, post, session_file};
use crate::yard_upgrade_totals::upgrade;

const MARKER: &str = "cacheWriteClassified: true\n";

/// A Codex request as an adapter that did not classify cache writes posted it, under `id`, with
/// `account` as its source's account where it gives one and `write` cache writes.
fn legacy(yard: &Yard, second: u32, id: &str, account: Option<&str>, write: u64) -> Value {
    let tokens = json!({"input":70,"cacheRead":30,"cacheWrite":write,"output":5});
    let mut row = event(
        yard,
        "request",
        second,
        &json!({"host":"codex","model":"fixture","eventId":id,"tokens":tokens}),
    );
    if let Some(account) = account {
        row["usageSource"] = json!({ "account": account });
    }
    row
}

fn lines(yard: &Yard) -> Vec<Value> {
    let path = yard.scratch.home().join(".yard/events/codex~s1.jsonl");
    let text = fs::read_to_string(path).unwrap();
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn totals(yard: &Yard) -> String {
    session_file(yard, "tokens", "codex~s1").unwrap()
}

/// A Codex session's events posted and counted, then its totals file left without the marker,
/// as a page server that did not classify cache writes left it, and the page server stopped.
fn legacy_session(rows: impl Fn(&Yard) -> Vec<Value>) -> Yard {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--host", "codex", "--send"]);
    let served = Served::start(&yard, &["--tree", yard.scratch.0.to_str().unwrap()]);
    let reply = post(&served, &rows(&yard));
    assert_eq!(reply["rejected"], json!([]));
    drop(served);
    let path = yard
        .folder(crate::yard::BRANCH)
        .join("tokens/codex~s1.yaml");
    fs::write(&path, totals(&yard).replacen(MARKER, "", 1)).unwrap();
    yard
}

/// Two requests whose zero cache writes no verified API-key account reported.
fn unclassified(yard: &Yard) -> Vec<Value> {
    vec![
        legacy(yard, 1, "one", Some("chatgpt"), 0),
        legacy(yard, 2, "two", None, 0),
    ]
}

#[test]
fn each_unverified_zero_moves_under_its_source_and_keeps_its_event_identity() {
    let yard = legacy_session(|yard| {
        let mut rows = unclassified(yard);
        rows.push(legacy(yard, 3, "three", Some("chatgpt"), 4));
        rows.push(legacy(yard, 4, "four", Some("apiKey"), 0));
        rows
    });
    let before = lines(&yard);
    upgrade(&yard);
    let after = lines(&yard);
    assert_eq!(after.len(), before.len());
    for (was, line) in before.iter().zip(&after) {
        assert_eq!(line["eventId"], was["eventId"]);
        assert_eq!(line["eventDigest"], was["eventDigest"]);
    }
    let reported = json!({"cacheWrite":0});
    assert!(after[0]["tokens"].get("cacheWrite").is_none());
    assert_eq!(
        after[0]["usageSource"],
        json!({"account":"chatgpt","reported":reported})
    );
    assert!(after[1]["tokens"].get("cacheWrite").is_none());
    assert_eq!(after[1]["usageSource"], json!({ "reported": reported }));
    assert_eq!(after[2], before[2], "a positive count stays where it is");
    assert_eq!(
        after[3], before[3],
        "an API-key account's zero stays measured"
    );
    let mut moved = after[0].clone();
    moved["tokens"]["cacheWrite"] = json!(0);
    moved["usageSource"] = json!({"account":"chatgpt"});
    assert_eq!(moved, before[0], "nothing else of the line changed");
}

/// Upgrades `yard`, whose last request ended at second 2, and checks that the page server,
/// replaying the session's events, writes the totals the upgrade wrote.
fn replay_gives_what_the_upgrade_wrote(yard: &Yard) {
    upgrade(yard);
    let written = totals(yard);
    assert!(written.contains(MARKER), "{written}");
    let served = Served::start(yard, &[]);
    // A request that adds nothing but its time makes the page server write the totals it
    // replayed.
    let nothing = json!({"input":0,"cacheRead":0,"output":0});
    let mut idle = event(
        yard,
        "request",
        5,
        &json!({"host":"codex","model":"fixture"}),
    );
    idle["tokens"] = nothing;
    post(&served, &[idle]);
    let replayed = totals(yard);
    let expected = written.replace(
        "lastEnded: 2026-01-01T10:00:02Z",
        "lastEnded: 2026-01-01T10:00:05Z",
    );
    assert_eq!(replayed, expected);
}

#[test]
fn replaying_the_session_after_the_upgrade_gives_the_totals_it_wrote() {
    replay_gives_what_the_upgrade_wrote(&legacy_session(unclassified));
}

#[test]
fn a_scope_with_a_zero_and_a_positive_cache_write_is_upgraded_as_replay_counts_it() {
    let yard = legacy_session(|yard| {
        vec![
            legacy(yard, 1, "one", Some("chatgpt"), 0),
            legacy(yard, 2, "two", Some("chatgpt"), 4),
        ]
    });
    replay_gives_what_the_upgrade_wrote(&yard);
    let written = totals(&yard);
    assert!(
        written.contains("tokens:\n  input: 140\n  cacheRead: 60\n  cacheWrite: 4\n  output: 10\n  partial:\n    - cacheWrite\n"),
        "{written}"
    );
}

#[test]
fn a_retry_of_an_original_event_is_still_recognized_after_the_upgrade() {
    let yard = legacy_session(unclassified);
    upgrade(&yard);
    let written = totals(&yard);
    let served = Served::start(&yard, &[]);
    let retry = legacy(&yard, 1, "one", Some("chatgpt"), 0);
    let reply = post(&served, &[retry]);
    assert_eq!(reply["rejected"], json!([]));
    assert_eq!(lines(&yard).len(), 2);
    assert_eq!(totals(&yard), written);
    let conflicting = legacy(&yard, 1, "one", Some("chatgpt"), 9);
    let reply = post(&served, &[conflicting]);
    assert_eq!(reply["rejected"][0]["index"], 0);
}

/// A Codex session's unverified zeros posted to a page server that did not move them, its
/// totals file left as that page server marked it.
fn marked_session() -> Yard {
    let yard = legacy_session(unclassified);
    let path = yard
        .folder(crate::yard::BRANCH)
        .join("tokens/codex~s1.yaml");
    let text = totals(&yard);
    fs::write(
        &path,
        text.replacen("host: codex\n", &format!("host: codex\n{MARKER}"), 1),
    )
    .unwrap();
    yard
}

#[test]
fn the_events_of_a_session_whose_totals_are_already_marked_are_moved_and_its_totals_kept() {
    let yard = marked_session();
    let marked = totals(&yard);
    assert!(marked.contains(MARKER), "{marked}");
    let (code, result) = upgrade(&yard);
    assert_eq!(code, Some(0));
    assert_eq!(result["classified"], json!([]));
    for line in lines(&yard) {
        assert!(line["tokens"].get("cacheWrite").is_none(), "{line}");
        assert_eq!(line["usageSource"]["reported"], json!({"cacheWrite":0}));
    }
    assert_eq!(
        totals(&yard),
        marked,
        "the marked totals file is left as it was"
    );
}

#[test]
fn a_second_run_moves_no_event() {
    let yard = marked_session();
    upgrade(&yard);
    let path = yard.scratch.home().join(".yard/events/codex~s1.jsonl");
    let first = fs::read(&path).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    upgrade(&yard);
    assert_eq!(fs::read(&path).unwrap(), first);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
}

#[test]
fn while_a_page_server_runs_a_marked_session_s_events_are_left_as_they_were() {
    let yard = marked_session();
    let path = yard.scratch.home().join(".yard/events/codex~s1.jsonl");
    let before = fs::read(&path).unwrap();
    let served = Served::start(&yard, &[]);
    let (code, result) = upgrade(&yard);
    assert_eq!(code, Some(0));
    assert_eq!(result["totalsLeft"]["port"], served.port);
    assert_eq!(fs::read(&path).unwrap(), before);
    drop(served);
    upgrade(&yard);
    assert_ne!(
        fs::read(&path).unwrap(),
        before,
        "a run after it stops moves them"
    );
}
