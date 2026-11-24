//! Connected mode: with `LOFT_YARD_SERVER_ENDPOINT` and `LOFT_YARD_SERVER_TOKEN` set, each
//! version an agent writes is copied into the outbox and sent to Yard Server by the command that
//! wrote it, once its result is printed; without them, Yard Client keeps Yard Log on the machine
//! alone. Each test points `loft` at the Software Plant's Yard Server simulator.

use std::fs;
use std::path::PathBuf;
use std::process::Output;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use software_plant::YardServer;
use software_plant::yard_server::{BATCHES, TOKEN};

use crate::common::stderr;
use crate::yard::{Yard, document};

pub const STAGE_BATCH: &str =
    r#"[{"id":"build","kind":"stage","title":"Build","status":"active"}]"#;

/// The variables that turn connected mode on for `endpoint`.
pub fn connected(endpoint: &str) -> [(&'static str, String); 2] {
    [
        ("LOFT_YARD_SERVER_ENDPOINT", endpoint.to_owned()),
        ("LOFT_YARD_SERVER_TOKEN", TOKEN.to_owned()),
    ]
}

pub fn pairs<'a>(variables: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    variables
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect()
}

/// `loft yard write` of `batch` as session `s1`, under `variables`.
pub fn write(yard: &Yard, batch: &str, variables: &[(&'static str, String)]) -> Output {
    yard.run_with_under(&["write", "--session", "s1"], batch, &pairs(variables))
}

/// `loft yard <args>`, under `variables`.
pub fn run(yard: &Yard, args: &[&str], variables: &[(&'static str, String)]) -> Output {
    yard.run_under(args, &pairs(variables))
}

/// The outbox of the machine `yard` runs on.
pub fn outbox(yard: &Yard) -> PathBuf {
    yard.scratch.home().join(".yard/outbox")
}

/// The entries waiting in the outbox, oldest first, read as JSON.
pub fn entries(yard: &Yard) -> Vec<Value> {
    let Ok(listing) = fs::read_dir(outbox(yard)) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = listing
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| serde_json::from_slice(&fs::read(path).unwrap()).unwrap())
        .collect()
}

/// The ids of the records a server holds, in the order it first received them.
pub fn held(server: &YardServer) -> Vec<String> {
    server
        .records()
        .iter()
        .map(|record| record["id"].as_str().unwrap().to_owned())
        .collect()
}

/// `loft yard flush` under `variables`: its exit status and its document.
pub fn flush(yard: &Yard, variables: &[(&'static str, String)]) -> (Option<i32>, Value) {
    let output = run(yard, &["flush"], variables);
    (output.status.code(), document("flush", &output))
}

/// Leaves one entry waiting in the outbox for the address `variables` names: the stage is
/// written offline, then 17 notes, of which the write sends 16.
pub fn one_waiting(yard: &Yard, variables: &[(&'static str, String)]) {
    write(yard, STAGE_BATCH, &[]);
    write(yard, &notes(17), variables);
    assert_eq!(entries(yard).len(), 1);
}

/// A batch of `count` notes in the stage `build`, `n1` to `n<count>`.
pub fn notes(count: usize) -> String {
    let notes: Vec<Value> = (1..=count)
        .map(|n| json!({"id": format!("n{n}"), "kind": "note", "stage": "build", "markdown": "x"}))
        .collect();
    Value::from(notes).to_string()
}

#[test]
fn a_write_sends_its_versions_once_its_result_is_printed_and_empties_the_outbox() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let output = write(&yard, STAGE_BATCH, &connected(&server.url()));
    let written = document("write", &output);
    assert_eq!(written["outcome"], "written", "{written}");
    assert_eq!(held(&server), ["build"]);
    assert_eq!(entries(&yard), Vec::<Value>::new());
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        (requests[0].method.as_str(), requests[0].path.as_str()),
        ("POST", BATCHES)
    );
    assert_eq!(
        requests[0].header("Authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert_eq!(requests[0].header("Content-Type"), Some("application/json"));
    let batch: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(batch["format"], "yardlog-batch/1");
}

#[test]
fn a_base_address_ending_in_a_slash_takes_one_slash_before_the_path() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    write(
        &yard,
        STAGE_BATCH,
        &connected(&format!("{}/", server.url())),
    );
    assert_eq!(server.requests()[0].path, BATCHES);
}

#[test]
fn without_both_variables_nothing_is_copied_and_nothing_is_sent() {
    let server = YardServer::accepting();
    for variables in [
        vec![],
        vec![("LOFT_YARD_SERVER_ENDPOINT", server.url())],
        vec![("LOFT_YARD_SERVER_TOKEN", TOKEN.to_owned())],
        vec![
            ("LOFT_YARD_SERVER_ENDPOINT", server.url()),
            ("LOFT_YARD_SERVER_TOKEN", String::new()),
        ],
        vec![
            ("LOFT_YARD_SERVER_ENDPOINT", String::new()),
            ("LOFT_YARD_SERVER_TOKEN", TOKEN.to_owned()),
        ],
    ] {
        let yard = Yard::new();
        let output = write(&yard, STAGE_BATCH, &variables);
        assert_eq!(document("write", &output)["outcome"], "written");
        assert_eq!(stderr(&output), "", "{variables:?}");
        assert!(!outbox(&yard).exists(), "{variables:?}");
    }
    assert_eq!(server.requests().len(), 0);
}

#[test]
fn a_write_gives_the_same_result_whether_yard_server_takes_the_batch_or_not() {
    let plain = Yard::new();
    let offline = document("write", &write(&plain, STAGE_BATCH, &[]));
    let failing = YardServer::failing(503);
    let refusing = YardServer::failing(401);
    for endpoint in [failing.url(), refusing.url(), YardServer::refusing()] {
        let yard = Yard::new();
        let output = write(&yard, STAGE_BATCH, &connected(&endpoint));
        assert_eq!(output.status.code(), Some(0));
        let written = document("write", &output);
        assert_eq!(written["outcome"], offline["outcome"]);
        assert_eq!(written["revision"], offline["revision"]);
        assert_eq!(entries(&yard).len(), 1, "{endpoint}");
        let said = stderr(&output);
        assert!(said.contains("did not reach Yard Server"), "{said}");
        assert_eq!(said.lines().count(), 1, "{said}");
    }
    assert_eq!(failing.requests().len(), 1);
    assert_eq!(refusing.requests().len(), 1);
}

#[test]
fn a_silent_yard_server_holds_a_write_at_most_two_seconds_and_the_entries_stay() {
    // The same write without connected mode is the baseline: what a silent Yard Server adds is
    // the wait alone, and a busy machine slows both runs alike.
    let plain = Yard::new();
    let started = Instant::now();
    write(&plain, STAGE_BATCH, &[]);
    let baseline = started.elapsed();
    let yard = Yard::new();
    let server = YardServer::silent();
    let started = Instant::now();
    let output = write(&yard, STAGE_BATCH, &connected(&server.url()));
    let took = started.elapsed();
    assert_eq!(document("write", &output)["outcome"], "written");
    assert!(took >= Duration::from_secs(2), "{took:?}");
    let added = took.saturating_sub(baseline);
    assert!(
        added < Duration::from_secs(3),
        "{added:?} over {baseline:?}"
    );
    assert_eq!(entries(&yard).len(), 1);
}

#[test]
fn an_http_address_off_the_loopback_interface_is_never_contacted_and_the_entries_stay() {
    let yard = Yard::new();
    let endpoint = "http://192.0.2.1:9";
    let output = write(&yard, STAGE_BATCH, &connected(endpoint));
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(entries(&yard).len(), 1);
    assert_eq!(entries(&yard)[0]["to"]["endpoint"], endpoint);
    let said = stderr(&output);
    assert!(said.contains(endpoint), "{said}");
}

#[test]
fn a_batch_takes_the_16_oldest_entries_and_the_newest_waits_for_the_next_sending() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &[]);
    write(&yard, &notes(17), &variables);
    let first: Vec<String> = (1..=16).map(|n| format!("n{n}")).collect();
    assert_eq!(held(&server), first);
    let left = entries(&yard);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0]["record"]["id"], "n17");
    let (_, flushed) = flush(&yard, &variables);
    assert_eq!(flushed["outcome"], "sent");
    assert_eq!(
        (&flushed["sent"], &flushed["remaining"]),
        (&json!(1), &json!(0))
    );
    assert_eq!(entries(&yard), Vec::<Value>::new());
    assert_eq!(held(&server).last().unwrap(), "n17");
}

#[test]
fn until_a_batch_is_taken_the_next_sending_takes_the_same_oldest_entries_again() {
    let yard = Yard::new();
    let server = YardServer::hanging_up();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    write(&yard, &notes(1), &variables);
    let requests = server.requests();
    let second: Value = serde_json::from_slice(&requests[1].body).unwrap();
    let ids: Vec<&str> = second["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| record["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["build", "n1"]);
    assert_eq!(held(&server), ["build", "n1"]);
}

#[test]
fn each_address_takes_only_its_own_entries() {
    let yard = Yard::new();
    let theirs = YardServer::failing(503);
    let ours = YardServer::accepting();
    write(&yard, STAGE_BATCH, &connected(&theirs.url()));
    write(&yard, &notes(1), &connected(&ours.url()));
    assert_eq!(held(&ours), ["n1"]);
    let left = entries(&yard);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0]["to"]["endpoint"], theirs.url());
}

#[test]
fn no_file_of_the_outbox_or_of_yard_log_holds_the_token() {
    let yard = Yard::new();
    let server = YardServer::failing(503);
    write(&yard, STAGE_BATCH, &connected(&server.url()));
    for dir in [outbox(&yard), yard.folder(crate::yard::BRANCH)] {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                let text = fs::read_to_string(&path).unwrap();
                assert!(!text.contains(TOKEN), "{}", path.display());
            }
        }
    }
}

#[test]
fn a_refused_write_and_an_unchanged_task_send_nothing() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(
        &yard,
        r#"[{"id":"build","kind":"stage","title":"Build","status":"active"},
            {"id":"t1","kind":"task","stage":"build","title":"T","state":"open"}]"#,
        &[],
    );
    // 17 versions: the write sends 16 and one waits, which a sending would take.
    write(&yard, &notes(17), &variables);
    assert_eq!(entries(&yard).len(), 1);
    let before = server.requests().len();
    let refused = write(
        &yard,
        r#"[{"id":"n1","kind":"note","stage":"missing","markdown":"x"}]"#,
        &variables,
    );
    assert_eq!(refused.status.code(), Some(1));
    let unchanged = run(
        &yard,
        &["task", "t1", "--state", "open", "--session", "s1"],
        &variables,
    );
    assert_eq!(document("task", &unchanged)["outcome"], "unchanged");
    assert_eq!(server.requests().len(), before);
    let changed = run(
        &yard,
        &["task", "t1", "--state", "active", "--session", "s1"],
        &variables,
    );
    assert_eq!(document("task", &changed)["outcome"], "changed");
    assert_eq!(server.requests().len(), before + 1);
}
