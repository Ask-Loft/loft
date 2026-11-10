//! `loft yard consultation`: the sums of the branch's consultation files, which the page server
//! writes from the mod's events, in all, by stage, by session and by agent type; nothing measured
//! where no file is; a closed Yard Log read as an open one; a table for a person; no file written.

use std::process::Output;

use serde_json::{Value, json};

use crate::served::Served;
use crate::snapshot::snapshot;
use crate::yard::{ASK, Yard, document};
use crate::yard_serve_consult::{described, started};
use crate::yard_serve_events::{event, keys, open, post, tokens};

fn report(yard: &Yard) -> Value {
    document("consultation", &yard.run(&["consultation"]))
}

/// A request of session `s1`'s main agent at `second`, its prompt holding `read` cached tokens.
fn request(yard: &Yard, second: u32, read: u64) -> Value {
    event(
        yard,
        "request",
        second,
        &json!({ "model": "opus", "tokens": tokens(10, read, 0, 5) }),
    )
}

/// The calls of a session that edited code before it read the specification, ran a `loft`
/// read, and had an Explore subagent read code, with the requests around them.
fn counted(yard: &Yard, served: &Served) {
    let mut sub = started(yard, "c4", 4, &json!({ "reads": ["code"] }));
    sub["agent"] = json!({ "id": "a1", "type": "Explore" });
    let edit = json!({ "edits": ["code"], "tool": "Edit" });
    let sent = post(
        served,
        &[
            described(yard, true),
            request(yard, 0, 100),
            started(yard, "c1", 1, &edit),
            event(yard, "tool.end", 1, &json!({ "call": "c1" })),
            started(yard, "c2", 2, &json!({ "reads": ["spec"] })),
            event(yard, "tool.end", 2, &json!({ "call": "c2" })),
            started(yard, "c3", 3, &json!({ "loft": ["find"], "tool": "Bash" })),
            sub,
            request(yard, 5, 500),
        ],
    );
    assert_eq!(sent["rejected"], json!([]), "{sent}");
}

#[test]
fn the_report_sums_the_files_in_all_by_stage_by_session_and_by_agent_type() {
    let (yard, served) = open();
    let (stage, _) = keys(&yard);
    counted(&yard, &served);
    let report = report(&yard);
    assert_eq!(report["measured"], true, "{report}");
    assert_eq!(report["partial"], false);
    let all = &report["all"]["counts"];
    let expected = json!({"specReads": 1, "codeReads": 1, "otherReads": 0, "loftReads": 1,
                          "specEdits": 0, "codeEdits": 1});
    assert_eq!(all, &expected);
    // 510 - (110 + 5) = 395 shared by the two calls that ended: the edit of code and the read
    // of the specification.
    assert_eq!(
        report["all"]["estimate"],
        json!({"spec": 197, "code": 198, "other": 0, "unclassified": 0})
    );
    let shape = &report["stages"][0];
    assert_eq!(shape["stage"], stage.as_str());
    assert_eq!(shape["title"], "Shape it");
    assert_eq!(shape["counts"], expected);
    assert_eq!(shape["firstCodeEdit"], "2026-01-01T10:00:01Z");
    assert_eq!(shape["firstSpec"], "2026-01-01T10:00:02Z");
    assert_eq!(shape["codeEditsBeforeSpec"], 1);
    assert_eq!(shape["specFirst"], false);
    let session = &report["sessions"][0];
    assert_eq!(
        (&session["session"], &session["host"]),
        (&json!("s1"), &json!("claude-code"))
    );
    assert_eq!(session["counts"], expected);
    let agents: Vec<&Value> = report["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| &a["type"])
        .collect();
    assert_eq!(agents, [&json!("main"), &json!("Explore")]);
    assert_eq!(report["agents"][1]["counts"]["codeReads"], 1);
    assert_eq!(report["unmeasured"], json!([]));
}

#[test]
fn a_stage_whose_code_was_edited_only_after_the_specification_says_it_was_consulted_first() {
    let (yard, served) = open();
    let edit = json!({ "edits": ["code"], "tool": "Edit" });
    post(
        &served,
        &[
            described(&yard, true),
            started(&yard, "c1", 1, &json!({ "reads": ["spec"] })),
            started(&yard, "c2", 2, &edit),
        ],
    );
    let shape = &report(&yard)["stages"][0];
    assert_eq!(shape["specFirst"], true, "{shape}");
    assert_eq!(shape["codeEditsBeforeSpec"], 0);
}

#[test]
fn an_unmeasured_session_is_listed_apart_and_a_partial_estimate_is_said_to_be() {
    let (yard, served) = open();
    let mut other = described(&yard, false);
    other["session"] = json!("s2");
    let lacking = event(
        &yard,
        "request",
        6,
        &json!({ "model": "opus", "tokens": {"input": 10} }),
    );
    counted(&yard, &served);
    post(&served, &[other, lacking]);
    let report = report(&yard);
    assert_eq!(
        report["unmeasured"],
        json!([{ "session": "s2", "host": "claude-code" }])
    );
    assert_eq!(report["partial"], true);
    assert_eq!(report["sessions"][0]["partial"], true);
    assert_eq!(report["sessions"].as_array().unwrap().len(), 1);
}

#[test]
fn a_branch_with_no_consultation_file_reports_nothing_measured_and_exits_0() {
    for yard in [Yard::new(), {
        let yard = Yard::new();
        yard.write("s1", ASK, &[]);
        yard
    }] {
        let output = yard.run(&["consultation"]);
        assert_eq!(output.status.code(), Some(0));
        let report = document("consultation", &output);
        assert_eq!(report["measured"], false, "{report}");
        assert!(report.get("all").is_none());
        assert_eq!(report["stages"], json!([]));
        assert_eq!(report["sessions"], json!([]));
    }
}

#[test]
fn a_closed_yard_log_is_reported_as_an_open_one() {
    let (yard, served) = open();
    counted(&yard, &served);
    let before = report(&yard);
    yard.loft(&["yard", "close", "--session", "s1"]);
    assert_eq!(report(&yard), before);
}

/// What `loft yard consultation` prints for a person in `yard`: its standard output and error.
fn text(yard: &Yard) -> (String, String) {
    let output = plain(yard);
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn plain(yard: &Yard) -> Output {
    let args = ["yard", "consultation"];
    yard.scratch.loft_with(&args, Some(&yard.git), &yard.clock)
}

#[test]
fn without_format_json_the_report_is_a_table_on_standard_error() {
    let (yard, served) = open();
    counted(&yard, &served);
    let (stdout, stderr) = text(&yard);
    assert_eq!(stdout, "");
    let lines: Vec<&str> = stderr.lines().collect();
    let cells = |values: [&str; 6]| values.map(|value| format!("{value:>12}")).concat();
    let head = ["spec", "loft", "code", "other", "spec edits", "code edits"];
    assert_eq!(
        lines[1],
        format!("{:<32}{}  spec first", "stage", cells(head))
    );
    let shape = cells(["1", "1", "1", "0", "0", "1"]);
    assert_eq!(
        lines[2],
        format!("{:<32}{shape}  no (1 before)", "Shape it")
    );
    assert!(
        stderr.contains("Estimated context tokens (an estimate): spec 197, code 198"),
        "{stderr}"
    );
    let (stdout, stderr) = text(&Yard::new());
    assert_eq!(stdout, "");
    assert!(
        stderr.starts_with("Nothing has been measured on feat/shelf"),
        "{stderr}"
    );
}

#[test]
fn no_file_is_written() {
    let (yard, served) = open();
    counted(&yard, &served);
    drop(served);
    let before = snapshot(&yard.scratch.0);
    let home = snapshot(&yard.scratch.home());
    report(&yard);
    plain(&yard);
    assert!(
        snapshot(&yard.scratch.0) == before,
        "a file of the tree changed"
    );
    assert!(
        snapshot(&yard.scratch.home()) == home,
        "a file of the home changed"
    );
}
