//! What a session's agents consulted, as the page server counts it from the mod's events into the
//! session's consultation file in Yard Log's folder: only under a descriptor that declares
//! `consults`, by stage and agent, with the estimate a request's prompt growth gives, written
//! without changing Yard Log's revision and rebuilt the same from the event log after a restart.

use serde_json::{Value, json};

use crate::served::Served;
use crate::yard::{ASK, BRANCH, Yard};
use crate::yard_serve_events::{dir, event, keys, open, post, session_file, tokens};

/// A descriptor that declares `consults`, or one that does not.
fn descriptor(consults: bool) -> Value {
    let mut adapter = crate::yard_capabilities::descriptor(&["input", "cacheRead", "output"]);
    if consults {
        adapter["capabilities"]["consults"] = json!(true);
    }
    adapter
}

/// The turn's start that carries the session's descriptor.
pub(crate) fn described(yard: &Yard, consults: bool) -> Value {
    event(
        yard,
        "turn.start",
        0,
        &json!({ "adapter": descriptor(consults) }),
    )
}

/// A tool call's start at `second`, with `more` fields.
pub(crate) fn started(yard: &Yard, call: &str, second: u32, more: &Value) -> Value {
    let mut fields = json!({ "call": call, "tool": "Read" });
    for (name, value) in more.as_object().unwrap() {
        fields[name] = value.clone();
    }
    event(yard, "tool.start", second, &fields)
}

fn consultation(yard: &Yard) -> Option<String> {
    session_file(yard, "consultation", "s1")
}

#[test]
fn a_session_s_calls_are_counted_in_all_by_stage_and_by_agent_and_change_no_revision() {
    let (yard, served) = open();
    let (stage, _) = keys(&yard);
    let revision = yard.read("s1", &[])["revision"].clone();
    let mut sub = started(&yard, "c4", 4, &json!({ "reads": ["code", "other"] }));
    sub["agent"] = json!({ "id": "a1", "type": "Explore" });
    let sent = post(
        &served,
        &[
            described(&yard, true),
            started(
                &yard,
                "c1",
                1,
                &json!({ "edits": ["code"], "tool": "Edit" }),
            ),
            started(
                &yard,
                "c2",
                2,
                &json!({ "loft": ["find", "fmt"], "tool": "Bash" }),
            ),
            started(&yard, "c3", 3, &json!({ "reads": ["spec"] })),
            sub,
            started(
                &yard,
                "c5",
                5,
                &json!({ "loft": ["publish"], "tool": "Bash" }),
            ),
        ],
    );
    assert_eq!(sent["recorded"], 6, "{sent}");
    let text = consultation(&yard).expect("the consultation file is written");
    let all = "counts:\n  specReads: 1\n  codeReads: 1\n  otherReads: 1\n  loftReads: 1\n  \
               specEdits: 0\n  codeEdits: 1\n";
    assert!(
        text.starts_with(
            "format: yardconsult/1\nsession: s1\nhost: claude-code\n\
             firstStarted: 2026-01-01T10:00:01Z\nlastStarted: 2026-01-01T10:00:04Z\n\
             lastEnded: 2026-01-01T10:00:04Z\n"
        ),
        "{text}"
    );
    assert!(text.contains(all), "{text}");
    assert!(text.contains(&format!("  - stage: {stage}\n")), "{text}");
    assert!(
        text.contains("firstCodeEdit: 2026-01-01T10:00:01Z\n"),
        "{text}"
    );
    assert!(text.contains("codeEditsBeforeSpec: 1\n"), "{text}");
    assert!(
        text.contains("agent: a1\n") && text.contains("type: Explore\n"),
        "{text}"
    );
    assert!(!text.contains("noStage"), "{text}");
    assert_eq!(yard.read("s1", &[])["revision"], revision);
}

#[test]
fn a_session_that_made_no_counted_call_writes_no_consultation_file() {
    let (yard, served) = open();
    post(
        &served,
        &[
            described(&yard, true),
            started(&yard, "c1", 1, &json!({})),
            started(&yard, "c2", 2, &json!({ "loft": ["fmt"], "tool": "Bash" })),
        ],
    );
    assert!(consultation(&yard).is_none());
}

#[test]
fn a_session_whose_descriptor_does_not_declare_consults_is_not_counted() {
    let (yard, served) = open();
    let sent = post(
        &served,
        &[
            described(&yard, false),
            started(
                &yard,
                "c1",
                1,
                &json!({ "reads": ["spec"], "loft": ["check"] }),
            ),
        ],
    );
    assert_eq!(sent["recorded"], 2, "{sent}");
    assert!(consultation(&yard).is_none());
    let (bare, served) = open();
    post(
        &served,
        &[started(&bare, "c1", 1, &json!({ "reads": ["spec"] }))],
    );
    assert!(consultation(&bare).is_none(), "no descriptor declares none");
}

#[test]
fn no_call_is_counted_once_yard_log_is_closed() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let served = Served::start(&yard, &[]);
    post(&served, &[described(&yard, true)]);
    yard.run(&["close", "--session", "s1"]);
    let sent = post(
        &served,
        &[started(&yard, "c1", 1, &json!({ "reads": ["spec"] }))],
    );
    assert_eq!(sent["skipped"], 1, "{sent}");
    assert!(consultation(&yard).is_none());
}

#[test]
fn a_call_whose_agent_works_in_another_worktree_is_counted_in_that_worktree_s_yard_log() {
    let (yard, served) = open();
    let other = Yard::new();
    other.write("s1", ASK, &[]);
    let mut elsewhere = started(&yard, "c1", 1, &json!({ "reads": ["spec"] }));
    elsewhere["dir"] = json!(dir(&other));
    elsewhere["agent"] = json!({ "id": "a1", "type": "Explore" });
    post(&served, &[described(&yard, true), elsewhere]);
    assert!(consultation(&yard).is_none());
    let text = session_file(&other, "consultation", "s1").unwrap();
    assert!(text.contains("  specReads: 1\n"), "{text}");
}

#[test]
fn the_estimate_goes_on_from_the_event_log_after_the_page_server_is_replaced() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let tree = yard.scratch.0.display().to_string();
    let served = Served::start(&yard, &["--tree", &tree]);
    let request = |second, read| {
        event(
            &yard,
            "request",
            second,
            &json!({ "model": "opus", "tokens": tokens(10, read, 0, 5) }),
        )
    };
    post(
        &served,
        &[
            described(&yard, true),
            request(1, 100),
            started(&yard, "c1", 2, &json!({ "reads": ["spec"] })),
        ],
    );
    let before = consultation(&yard).unwrap();
    assert!(!before.contains("estimate:"), "{before}");
    drop(served);
    // The call started before the restart; its end and the request after it come after.
    let served = Served::start(&yard, &["--tree", &tree]);
    post(
        &served,
        &[
            event(&yard, "tool.end", 3, &json!({ "call": "c1" })),
            request(4, 400),
        ],
    );
    let text = consultation(&yard).unwrap();
    // 410 - (110 + 5) = 295 brought in by the one call, which read the specification.
    assert!(text.contains("estimate:\n  spec: 295\n"), "{text}");
    assert!(text.contains("lastEnded: 2026-01-01T10:00:04Z\n"), "{text}");
    drop(served);
    // A third page server goes on from what the event log rebuilds: the counts so far, and the
    // last request's prompt the next one is measured against.
    let served = Served::start(&yard, &["--tree", &tree]);
    post(
        &served,
        &[
            started(&yard, "c2", 5, &json!({ "reads": ["code"] })),
            event(&yard, "tool.end", 6, &json!({ "call": "c2" })),
            request(7, 600),
        ],
    );
    let text = consultation(&yard).unwrap();
    // 610 - (410 + 5) = 195 brought in by the call that read code.
    assert!(
        text.contains("counts:\n  specReads: 1\n  codeReads: 1\n"),
        "{text}"
    );
    assert!(
        text.contains("estimate:\n  spec: 295\n  code: 195\n"),
        "{text}"
    );
}

/// A subagent's model request at `second` whose prompt holds `read` cached tokens, posted from
/// the directory of `yard`.
fn sub_request(yard: &Yard, from: &Yard, second: u32, read: u64) -> Value {
    let mut request = event(
        yard,
        "request",
        second,
        &json!({ "model": "opus", "tokens": tokens(10, read, 0, 5) }),
    );
    request["dir"] = json!(dir(from));
    request["agent"] = json!({ "id": "a1", "type": "Explore" });
    request
}

/// A subagent's call at `second` that reads `class`, started and ended in `yard`.
fn sub_call(yard: &Yard, call: &str, second: u32, class: &str) -> [Value; 2] {
    let mut start = started(yard, call, second, &json!({ "reads": [class] }));
    let mut end = event(yard, "tool.end", second + 1, &json!({ "call": call }));
    for event in [&mut start, &mut end] {
        event["agent"] = json!({ "id": "a1", "type": "Explore" });
    }
    [start, end]
}

#[test]
fn a_request_is_measured_against_its_agent_s_previous_request_whichever_yard_log_kept_it() {
    let (yard, served) = open();
    let other = Yard::new();
    other.write("s1", ASK, &[]);
    let mut events = vec![
        described(&yard, true),
        sub_request(&yard, &yard, 1, 100),
        sub_request(&yard, &other, 2, 1000),
    ];
    events.extend(sub_call(&yard, "c1", 3, "spec"));
    events.push(sub_request(&yard, &yard, 5, 1100));
    let sent = post(&served, &events);
    assert_eq!(sent["recorded"], 6, "{sent}");
    let text = consultation(&yard).unwrap();
    // 1110 - (1010 + 5) = 95, measured against the request the other Yard Log kept.
    assert!(text.contains("estimate:\n  spec: 95\n"), "{text}");
}

#[test]
fn a_request_after_one_no_yard_log_kept_attributes_nothing() {
    let (yard, served) = open();
    let nowhere = Yard::new();
    let mut events = vec![
        described(&yard, true),
        sub_request(&yard, &yard, 1, 100),
        sub_request(&yard, &nowhere, 2, 1000),
    ];
    events.extend(sub_call(&yard, "c1", 3, "spec"));
    events.push(sub_request(&yard, &yard, 5, 1100));
    events.extend(sub_call(&yard, "c2", 6, "code"));
    events.push(sub_request(&yard, &yard, 8, 1200));
    let sent = post(&served, &events);
    assert_eq!(sent["skipped"], 1, "{sent}");
    let text = consultation(&yard).unwrap();
    // The request after the skipped one has no earlier one; the next: 1210 - (1110 + 5) = 95.
    assert!(text.contains("estimate:\n  code: 95\n"), "{text}");
}

#[test]
fn a_consultation_file_of_another_format_keeps_its_bytes_while_the_session_s_calls_go_on() {
    let (yard, served) = open();
    let path = yard.folder(BRANCH).join("consultation/s1.yaml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let newer = "format: yardconsult/2\nsession: s1\nhost: claude-code\n";
    std::fs::write(&path, newer).unwrap();
    let sent = post(
        &served,
        &[
            described(&yard, true),
            started(&yard, "c1", 1, &json!({ "reads": ["spec"] })),
        ],
    );
    assert_eq!(sent["recorded"], 2, "{sent}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), newer);
}

#[test]
fn the_page_server_gives_the_page_the_sums_as_the_command_reports_them() {
    let (yard, served) = open();
    let tree = yard.scratch.0.canonicalize().unwrap().display().to_string();
    let log = || {
        let reply = served.get(&format!("/api/log?tree={tree}&branch={BRANCH}"));
        let schema =
            std::fs::read_to_string(crate::common::workspace().join("schemas/yard-read.json"))
                .unwrap();
        crate::schema::conforming(&reply.body, &schema)
    };
    assert_eq!(log()["consultationSums"]["measured"], false);
    post(
        &served,
        &[
            described(&yard, true),
            started(&yard, "c1", 1, &json!({ "reads": ["spec"] })),
        ],
    );
    let sums = log()["consultationSums"].clone();
    assert_eq!(sums["stages"][0]["title"], "Shape it", "{sums}");
    assert_eq!(sums["all"]["counts"]["specReads"], 1);
    let reported = yard.run(&["consultation"]);
    let mut reported: Value = serde_json::from_slice(&reported.stdout).unwrap();
    for field in ["schema", "outcome", "errors", "tree", "branch", "commonDir"] {
        reported.as_object_mut().unwrap().remove(field);
    }
    assert_eq!(sums, reported);
}
