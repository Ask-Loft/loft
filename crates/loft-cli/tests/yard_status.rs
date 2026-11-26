//! `loft yard status`: whether the versions agents write leave the machine, and where they do not,
//! why. It reads the outbox, `dead/` and `~/.yard/flush.json`, makes no request, writes nothing
//! and exits 0 whatever it finds.

use std::fs;
use std::process::Output;

use serde_json::{Value, json};
use software_plant::yard_server::TOKEN;
use software_plant::{Clock, YardServer};

use crate::common::{EPOCH, Scratch};
use crate::snapshot::snapshot;
use crate::yard::{Yard, document};
use crate::yard_connected::{
    STAGE_BATCH, come_back_after, connected, dated, flush, flush_json, flush_record, keys, notes,
    outbox, pairs, put_off_record, system_now, write,
};

/// `loft yard status` under `variables`, as text and as JSON.
fn status(yard: &Yard, variables: &[(&'static str, String)]) -> (Output, Value) {
    let text = yard
        .scratch
        .command(Some(&yard.git), &yard.clock)
        .envs(pairs(variables))
        .args(["yard", "status"])
        .output()
        .unwrap();
    let json = yard.run_under(&["status"], &pairs(variables));
    assert_eq!(json.status.code(), Some(0));
    (text, document("status", &json))
}

/// The lines `loft yard status` printed.
fn lines(output: &Output) -> Vec<String> {
    assert_eq!(output.status.code(), Some(0));
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The line that starts with `key: `.
fn line(output: &Output, key: &str) -> String {
    lines(output)
        .into_iter()
        .find(|line| line.starts_with(&format!("{key}: ")))
        .unwrap_or_else(|| panic!("no line {key}: in {:?}", lines(output)))
}

/// Puts the file `name` into `dead/`, with `reason` beside it where one is given.
fn dead(yard: &Yard, name: &str, reason: Option<&str>) {
    let dir = outbox(yard).join("dead");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(name), "{}").unwrap();
    if let Some(reason) = reason {
        fs::write(dir.join(format!("{name}.reason")), format!("{reason}\n")).unwrap();
    }
}

#[test]
fn with_connected_mode_on_status_names_the_address_and_counts_what_waits_for_it() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    write(&yard, &notes(2), &variables);
    let (text, json) = status(&yard, &variables);
    assert_eq!(
        line(&text, "connected mode"),
        format!("connected mode: on, sending to {}", server.url())
    );
    assert_eq!(line(&text, "waiting"), "waiting: 3 for this address");
    assert_eq!(
        (&json["connected"], &json["endpoint"]),
        (&json!(true), &json!(server.url()))
    );
    assert_eq!(json["missing"], json!([]));
    assert_eq!(json["waiting"]["forAddress"], 3);
    assert_eq!(server.requests().len(), 0);
    let said = String::from_utf8_lossy(&text.stdout).into_owned() + &json.to_string();
    assert!(!said.contains(TOKEN), "{said}");
}

#[test]
fn with_neither_variable_set_connected_mode_is_off() {
    let yard = Yard::new();
    let (text, json) = status(&yard, &[]);
    assert_eq!(line(&text, "connected mode"), "connected mode: off");
    assert_eq!(json["connected"], false);
    assert!(json.get("endpoint").is_none(), "{json}");
    assert_eq!(
        json["missing"],
        json!(["LOFT_YARD_SERVER_ENDPOINT", "LOFT_YARD_SERVER_TOKEN"])
    );
    assert!(
        !lines(&text)
            .iter()
            .any(|line| line.starts_with("last attempt"))
    );
}

#[test]
fn with_one_variable_unset_or_empty_status_names_the_one_missing() {
    for (variables, missing) in [
        (
            vec![("LOFT_YARD_SERVER_ENDPOINT", "http://127.0.0.1:1".to_owned())],
            "LOFT_YARD_SERVER_TOKEN",
        ),
        (
            vec![
                ("LOFT_YARD_SERVER_ENDPOINT", String::new()),
                ("LOFT_YARD_SERVER_TOKEN", TOKEN.to_owned()),
            ],
            "LOFT_YARD_SERVER_ENDPOINT",
        ),
    ] {
        let yard = Yard::new();
        let (text, json) = status(&yard, &variables);
        assert_eq!(
            line(&text, "connected mode"),
            format!("connected mode: off; {missing} is unset or empty")
        );
        assert_eq!(json["missing"], json!([missing]));
    }
}

#[test]
fn an_address_the_network_client_refuses_is_named_as_refused() {
    let yard = Yard::new();
    let endpoint = "http://192.0.2.1:9";
    let (text, json) = status(&yard, &connected(endpoint));
    let said = line(&text, "connected mode");
    assert!(said.contains(endpoint), "{said}");
    assert!(said.contains("refused"), "{said}");
    assert_eq!(json["refused"], true);
}

#[test]
fn with_both_variables_set_and_no_home_directory_known_status_says_so() {
    let yard = Yard::new();
    let output = yard
        .scratch
        .command(Some(&yard.git), &yard.clock)
        .envs(connected("http://127.0.0.1:1"))
        .env("HOME", "not-an-absolute-home")
        .args(["yard", "status"])
        .output()
        .unwrap();
    let said = line(&output, "connected mode");
    assert!(said.contains("no home directory is known"), "{said}");
}

#[test]
fn waiting_counts_the_entries_for_other_addresses_put_off_and_of_a_newer_loft() {
    let yard = Yard::new();
    let theirs = YardServer::accepting();
    write(&yard, STAGE_BATCH, &connected(&theirs.url()));
    write(&yard, &notes(2), &connected("http://127.0.0.1:1"));
    let key = keys(&yard)[2].clone();
    let record = json!({ "http://127.0.0.1:1": put_off_record(&key) });
    fs::write(flush_json(&yard), record.to_string()).unwrap();
    fs::write(
        outbox(&yard).join("00000000-0000-7000-8000-000000000000.json"),
        json!({"format": "yardlog-outbox/9"}).to_string(),
    )
    .unwrap();
    let (text, json) = status(&yard, &connected("http://127.0.0.1:1"));
    assert_eq!(
        line(&text, "waiting"),
        "waiting: 2 for this address, 1 of them later; 1 for other addresses; 1 of a newer loft"
    );
    assert_eq!(
        (
            &json["waiting"]["forAddress"],
            &json["waiting"]["otherAddresses"],
            &json["waiting"]["later"],
            &json["waiting"]["skipped"]
        ),
        (&json!(2), &json!(1), &json!(1), &json!(1))
    );
}

#[test]
fn an_entry_counts_as_later_by_the_record_of_the_address_it_is_for_whatever_the_mode() {
    let yard = Yard::new();
    write(&yard, STAGE_BATCH, &connected("http://127.0.0.1:1"));
    write(&yard, &notes(1), &connected("http://127.0.0.1:2"));
    let theirs = &keys(&yard)[1];
    let records = json!({
        "http://127.0.0.1:1": put_off_record(theirs),
        "http://127.0.0.1:2": put_off_record(theirs),
    });
    fs::write(flush_json(&yard), records.to_string()).unwrap();
    let (text, json) = status(&yard, &[]);
    assert_eq!(
        line(&text, "waiting"),
        "waiting: 1 for http://127.0.0.1:1, 1 for http://127.0.0.1:2, 1 of them later"
    );
    assert_eq!(json["waiting"]["later"], json!(1));
    let (text, json) = status(&yard, &connected("http://127.0.0.1:1"));
    assert_eq!(
        line(&text, "waiting"),
        "waiting: 1 for this address; 1 for other addresses, 1 of them later"
    );
    assert_eq!(json["waiting"]["later"], json!(1));
    assert_eq!(
        json["waiting"]["byAddress"],
        json!([
            {"address": "http://127.0.0.1:1", "waiting": 1, "later": 0},
            {"address": "http://127.0.0.1:2", "waiting": 1, "later": 1},
        ])
    );
}

#[test]
fn with_connected_mode_off_waiting_counts_every_entry_with_its_address() {
    let yard = Yard::new();
    write(&yard, STAGE_BATCH, &connected("http://127.0.0.1:1"));
    write(&yard, &notes(2), &connected("http://127.0.0.1:2"));
    let (text, json) = status(&yard, &[]);
    assert_eq!(
        line(&text, "waiting"),
        "waiting: 1 for http://127.0.0.1:1, 2 for http://127.0.0.1:2"
    );
    assert_eq!(
        json["waiting"]["byAddress"],
        json!([
            {"address": "http://127.0.0.1:1", "waiting": 1, "later": 0},
            {"address": "http://127.0.0.1:2", "waiting": 2, "later": 0},
        ])
    );
}

#[test]
fn with_no_entry_waiting_the_line_reads_waiting_none() {
    let yard = Yard::new();
    let (text, _) = status(&yard, &connected("http://127.0.0.1:1"));
    assert_eq!(line(&text, "waiting"), "waiting: none");
}

#[test]
fn last_attempt_gives_when_the_last_run_ended_how_and_why() {
    let yard = Yard::new();
    let failing = YardServer::failing(503);
    let variables = connected(&failing.url());
    write(&yard, STAGE_BATCH, &variables);
    flush(&yard, &variables);
    let ended = flush_record(&yard, &failing.url())["ended"].clone();
    let (text, json) = status(&yard, &variables);
    assert_eq!(
        line(&text, "last attempt"),
        format!(
            "last attempt: {} failed: Yard Server refused the batch with 503 `service_unavailable`",
            ended.as_str().unwrap()
        )
    );
    assert_eq!(
        json["lastAttempt"],
        json!({
            "ended": ended,
            "outcome": "failed",
            "why": "Yard Server refused the batch with 503 `service_unavailable`",
            "notBefore": null,
        })
    );
}

#[test]
fn last_attempt_gives_the_time_to_come_back_after_while_it_has_not_passed() {
    let yard = Yard::new();
    let server = YardServer::coming_back(429, "120");
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    flush(&yard, &variables);
    let record = flush_record(&yard, &server.url());
    let (ended, not_before) = (record["ended"].as_str().unwrap(), &record["notBefore"]);
    let (text, _) = status(&yard, &variables);
    assert_eq!(
        line(&text, "last attempt"),
        format!(
            "last attempt: {ended} paused; Yard Server asked to come back after {}",
            not_before.as_str().unwrap()
        )
    );
    let passed = system_now() - 1;
    come_back_after(&yard, &server.url(), passed);
    let (text, json) = status(&yard, &variables);
    assert_eq!(
        line(&text, "last attempt"),
        format!("last attempt: {ended} paused")
    );
    assert_eq!(json["lastAttempt"]["notBefore"], dated(passed));
}

#[test]
fn with_no_run_recorded_or_a_flush_json_that_does_not_read_last_attempt_is_none() {
    let yard = Yard::new();
    let variables = connected("http://127.0.0.1:1");
    let (text, json) = status(&yard, &variables);
    assert_eq!(line(&text, "last attempt"), "last attempt: none");
    assert_eq!(json["lastAttempt"], Value::Null);
    let home = yard.scratch.home().join(".yard");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("flush.json"), "{not json").unwrap();
    let (text, _) = status(&yard, &variables);
    assert_eq!(line(&text, "last attempt"), "last attempt: none");
}

#[test]
fn dead_lists_each_file_with_its_reason_in_the_order_of_their_names() {
    let yard = Yard::new();
    dead(&yard, "b.json", Some("refused invalid_record"));
    dead(&yard, "a.json", Some("conflict"));
    dead(&yard, "c.json", None);
    fs::write(outbox(&yard).join("dead/gone.json.reason"), "conflict\n").unwrap();
    let (text, json) = status(&yard, &[]);
    let all = lines(&text);
    let at = all
        .iter()
        .position(|line| line.starts_with("dead: "))
        .unwrap();
    assert_eq!(
        all[at..],
        [
            "dead: 3",
            "dead a.json: conflict",
            "dead b.json: refused invalid_record",
            "dead c.json: unknown"
        ]
    );
    assert_eq!(
        json["dead"],
        json!([
            {"file": "a.json", "reason": "conflict"},
            {"file": "b.json", "reason": "refused invalid_record"},
            {"file": "c.json", "reason": "unknown"},
        ])
    );
}

#[test]
fn with_no_file_in_dead_the_line_reads_dead_none_and_no_file_line_follows() {
    let yard = Yard::new();
    let (text, json) = status(&yard, &[]);
    assert_eq!(line(&text, "dead"), "dead: none");
    assert!(!lines(&text).iter().any(|line| line.starts_with("dead ")));
    assert_eq!(json["dead"], json!([]));
}

#[test]
fn status_exits_0_makes_no_request_and_changes_no_file_whatever_it_finds() {
    let yard = Yard::new();
    let failing = YardServer::failing(503);
    let variables = connected(&failing.url());
    write(&yard, STAGE_BATCH, &variables);
    flush(&yard, &variables);
    dead(&yard, "a.json", Some("conflict"));
    let home = yard.scratch.home().join(".yard");
    let before = snapshot(&home);
    let requests = failing.requests().len();
    let (text, _) = status(&yard, &variables);
    assert_eq!(text.status.code(), Some(0));
    assert_eq!(failing.requests().len(), requests);
    assert_eq!(snapshot(&home), before);
}

#[test]
fn status_runs_outside_a_repository_without_a_session() {
    let scratch = Scratch::empty();
    let output = scratch
        .command(None, &Clock::fixed(EPOCH))
        .envs(connected("http://127.0.0.1:1"))
        .args(["yard", "status", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(document("status", &output)["outcome"], "reported");
}

#[test]
fn with_connected_mode_off_and_no_home_directory_known_status_says_no_home_is_known() {
    let yard = Yard::new();
    let output = yard
        .scratch
        .command(Some(&yard.git), &yard.clock)
        .env("HOME", "not-an-absolute-home")
        .args(["yard", "status", "--format", "json"])
        .output()
        .unwrap();
    let json = document("status", &output);
    assert_eq!(
        (&json["connected"], &json["homeKnown"]),
        (&json!(false), &json!(false))
    );
}
