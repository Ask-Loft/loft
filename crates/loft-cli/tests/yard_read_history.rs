//! Optional submission history gives a monitor authoritative terminal state without a lease.

use serde_json::json;

use crate::yard::{ASK, Yard, document};

fn question(id: &str) -> String {
    format!(
        r#"[{{"id":"{id}","kind":"question","stage":"shape","prompt":"Which?",
        "header":"Shelf","options":[{{"label":"Top"}},{{"label":"Bottom"}}]}}]"#
    )
}

#[test]
fn submission_history_retains_answered_withdrawn_and_open_submissions() {
    let yard = Yard::new();
    let answered = yard.write("s1", ASK, &["--send"])["submission"].clone();
    yard.answer("s1", r#"[{"block":"q1","labels":["Top"]}]"#);
    let withdrawn = yard.write("s1", &question("q2"), &["--send"])["submission"].clone();
    let output = yard.run(&[
        "withdraw",
        "--session",
        "s1",
        "--submission",
        withdrawn.as_str().unwrap(),
    ]);
    assert!(output.status.success());
    let open = yard.write("s1", &question("q3"), &["--send"])["submission"].clone();
    let read = yard.read("s1", &["--submissions"]);
    assert_eq!(read["schema"], "loft-yard-read/3");
    let sent = read["sent"].as_array().unwrap();
    assert_eq!(sent.len(), 3);
    for (submission, key, state) in [
        (&sent[0], answered, "answered"),
        (&sent[1], withdrawn, "withdrawn"),
        (&sent[2], open.clone(), "open"),
    ] {
        assert_eq!(submission["key"], key);
        assert_eq!(submission["state"], state);
        assert_eq!(submission["agent"]["session"], "s1");
        assert_eq!(submission["blocks"].as_array().unwrap().len(), 1);
        assert!(submission["reached"].as_u64().unwrap() > 0);
        assert_eq!(submission["changes"][0]["what"], "sent");
    }
    assert_eq!(sent[0]["changes"][1]["what"], "answered");
    assert_eq!(sent[0]["changes"][1]["act"]["revision"], 2);
    assert_eq!(read["submissions"][0]["open"][0]["key"], open);
}

#[test]
fn history_is_opt_in_and_a_since_filter_does_not_hide_resolved_submissions() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    yard.answer("s1", r#"[{"block":"q1","labels":["Top"]}]"#);
    let plain = yard.read("s1", &[]);
    assert!(plain.get("sent").is_none());
    assert!(plain.get("ownLastRead").is_none());
    let history = yard.read("s1", &["--submissions", "--since", "2"]);
    assert_eq!(history["stages"], json!([]));
    assert_eq!(history["sent"][0]["state"], "answered");
}

#[test]
fn own_read_progress_is_distinct_from_another_sessions_fallback_revision() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.read("s2", &["--mark-read"]);
    let before = yard.read("s1", &["--submissions"]);
    assert_eq!(before["lastRead"], 1);
    assert!(before["ownLastRead"].is_null());
    yard.read("s1", &["--mark-read"]);
    let after = yard.read("s1", &["--submissions"]);
    assert_eq!(after["ownLastRead"], 1);
}

#[test]
fn explicit_read_markers_are_isolated_by_host_and_subagent() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.read("s1", &["--host", "codex", "--mark-read"]);
    assert!(yard.read("s1", &["--submissions"])["ownLastRead"].is_null());
    let subagent = ["--host", "codex", "--agent", "a1", "--agent-type", "review"];
    let mut flags = subagent.to_vec();
    flags.push("--submissions");
    assert!(yard.read("s1", &flags)["ownLastRead"].is_null());
    let mut mark = subagent.to_vec();
    mark.push("--mark-read");
    yard.read("s1", &mark);
    assert_eq!(yard.read("s1", &flags)["ownLastRead"], 1);
}

#[test]
fn an_absent_log_has_empty_history_and_no_explicit_read_marker() {
    let yard = Yard::new();
    let read = yard.read("s1", &["--submissions"]);
    assert_eq!(read["sent"], json!([]));
    assert!(read.get("ownLastRead").is_some());
    assert!(read["ownLastRead"].is_null());
    let anonymous = document("read", &yard.run(&["read", "--submissions"]));
    assert_eq!(anonymous["sent"], json!([]));
    assert!(anonymous["ownLastRead"].is_null());
}

#[test]
fn a_detached_read_has_empty_history_without_writing_a_log() {
    let mut yard = Yard::new();
    yard.git = yard.git.clone().detached();
    let read = yard.read("s1", &["--submissions"]);
    assert_eq!(read["sent"], json!([]));
    assert!(read.get("ownLastRead").is_some());
    assert!(read["ownLastRead"].is_null());
    assert_eq!(read["revision"], 0);
    assert_eq!(read["detached"], true);
}
