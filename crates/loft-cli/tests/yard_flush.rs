//! `loft yard flush`, and who else sends: the command runs the sending, by hand and anywhere,
//! and names its outcome; the writing commands, the hooks of the agent hosts and the page server
//! never send.

use std::time::Duration;

use serde_json::json;
use software_plant::YardServer;

use crate::common::Scratch;
use crate::yard::{Yard, document};
use crate::yard_codex::{event, plan_update};
use crate::yard_connected::{
    STAGE_BATCH, connected, entries, flush, held, one_waiting, outbox, pairs, run, write,
};

#[test]
fn without_connected_mode_a_flush_is_off_and_makes_no_request() {
    let yard = Yard::new();
    let (code, flushed) = flush(&yard, &[]);
    assert_eq!(code, Some(0));
    assert_eq!(flushed["outcome"], "off");
    assert_eq!(
        (
            &flushed["sent"],
            &flushed["remaining"],
            &flushed["skipped"],
            &flushed["dead"]
        ),
        (&json!(0), &json!(0), &json!(0), &json!(0))
    );
}

#[test]
fn with_no_entry_for_the_address_a_flush_is_empty_and_makes_no_request() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let (code, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("empty")));
    assert_eq!(server.requests().len(), 0);
}

#[test]
fn a_flush_sends_what_a_write_left_and_counts_it() {
    let yard = Yard::new();
    let failing = YardServer::failing(503);
    write(&yard, STAGE_BATCH, &connected(&failing.url()));
    let (code, failed) = flush(&yard, &connected(&failing.url()));
    assert_eq!((code, &failed["outcome"]), (Some(1), &json!("failed")));
    assert_eq!(
        (&failed["sent"], &failed["remaining"]),
        (&json!(0), &json!(1))
    );
    let message = failed["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("503"), "{message}");
    assert!(message.contains("service_unavailable"), "{message}");
    let silent = YardServer::silent();
    write(&yard, STAGE_BATCH, &connected(&silent.url()));
    let (code, quiet) = flush(&yard, &connected(&silent.url()));
    assert_eq!(code, Some(1));
    let message = quiet["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("did not answer"), "{message}");
    assert_eq!(held(&failing), Vec::<String>::new());
    assert_eq!(entries(&yard).len(), 2);
}

#[test]
fn a_flush_runs_outside_a_repository_without_a_session_and_writes_no_yard_log() {
    let scratch = Scratch::empty();
    let server = YardServer::accepting();
    let output = scratch
        .command(None, &software_plant::Clock::fixed(crate::common::EPOCH))
        .envs(connected(&server.url()))
        .args(["yard", "flush", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(document("flush", &output)["outcome"], "empty");
    assert!(!scratch.0.join(".yard").exists());
}

#[test]
fn a_flush_counts_and_moves_aside_what_it_cannot_send() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let dir = outbox(&yard);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("00000000-0000-7000-8000-000000000001.json"),
        "{broken",
    )
    .unwrap();
    let newer = json!({"format": "yardlog-outbox/2", "to": {"endpoint": server.url()}});
    std::fs::write(
        dir.join("00000000-0000-7000-8000-000000000002.json"),
        newer.to_string(),
    )
    .unwrap();
    let (code, flushed) = flush(&yard, &connected(&server.url()));
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("empty")));
    assert_eq!(
        (&flushed["skipped"], &flushed["dead"]),
        (&json!(1), &json!(1))
    );
    assert!(
        dir.join("dead/00000000-0000-7000-8000-000000000001.json")
            .is_file()
    );
}

#[test]
fn the_codex_hook_puts_entries_and_sends_none_and_a_flush_takes_them() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    yard.run_with_under(
        &["write", "--session", "codex-session", "--host", "codex"],
        r#"[{"id":"build","kind":"stage","title":"Build","status":"active"}]"#,
        &[],
    );
    let mut input = event(&yard, "PostToolUse");
    input["tool_use_id"] = json!("plan-call");
    let input = plan_update(input, &[("Build", "in_progress")]);
    let pairs = pairs(&variables);
    let hooked = yard.run_with_under(&["codex", "hook"], &input.to_string(), &pairs);
    assert!(hooked.status.success());
    assert_eq!(server.requests().len(), 0);
    let ours: Vec<_> = entries(&yard)
        .into_iter()
        .filter(|entry| entry["to"]["endpoint"] == server.url())
        .collect();
    assert_eq!(ours.len(), 1);
    assert_eq!(ours[0]["record"]["kind"], "task");
    flush(&yard, &variables);
    let kinds: Vec<String> = server
        .records()
        .iter()
        .map(|record| record["kind"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(kinds, ["task"]);
}

#[test]
fn the_page_server_makes_no_request_to_yard_server_with_entries_waiting() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    one_waiting(&yard, &variables);
    let before = server.requests().len();
    let pairs = pairs(&variables);
    let page_server =
        crate::served::Served::start_with(&yard, crate::served::free_port(), &pairs, &[]);
    assert_eq!(page_server.get("/").status, 200);
    drop(page_server);
    assert_eq!(server.requests().len(), before);
    assert_eq!(entries(&yard).len(), 1);
}

#[test]
fn an_answer_that_arrives_too_late_leaves_the_batch_to_be_sent_again_and_stored_once() {
    let yard = Yard::new();
    let server = YardServer::late(Duration::from_secs(3));
    let variables = connected(&server.url());
    let output = write(&yard, STAGE_BATCH, &variables);
    assert_eq!(document("write", &output)["outcome"], "written");
    let (code, _) = flush(&yard, &variables);
    assert_eq!(code, Some(1));
    assert_eq!(held(&server), ["build"]);
    assert_eq!(entries(&yard).len(), 1);
    let (code, _) = flush(&yard, &variables);
    assert_eq!(code, Some(1));
    assert_eq!(server.requests().len(), 2);
    assert_eq!(held(&server), ["build"]);
}

#[test]
fn a_recorded_publish_puts_its_version_and_a_flush_sends_it() {
    let mut yard = Yard::with_spec();
    let on = |branch: &str| {
        software_plant::GitScenario::clean_on(branch)
            .base("HEAD", crate::yard::HEAD)
            .with_config("user.name", "Ada Reader")
            .with_config("user.email", "ada@example.com")
    };
    yard.git = on(crate::yard::BRANCH);
    yard.write("s1", crate::yard::ASK, &[]);
    yard.run(&["close", "--session", "s1"]);
    yard.git = on("develop");
    let server = YardServer::accepting();
    let output = run(
        &yard,
        &[
            "published",
            "--branch",
            crate::yard::BRANCH,
            "--session",
            "s2",
        ],
        &connected(&server.url()),
    );
    assert_eq!(document("published", &output)["outcome"], "recorded");
    assert_eq!(server.requests().len(), 0);
    flush(&yard, &connected(&server.url()));
    assert_eq!(held(&server), ["published"]);
    assert_eq!(entries(&yard), Vec::<serde_json::Value>::new());
}

#[test]
fn the_claude_hook_makes_no_request_to_yard_server_with_entries_waiting() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    one_waiting(&yard, &variables);
    let before = server.requests().len();
    let pairs = pairs(&variables);
    let mut start = crate::yard_claude_hook::input(&yard, "SessionStart", "s1");
    start["source"] = json!("startup");
    let hooked = yard.run_with_under(&["claude", "hook"], &start.to_string(), &pairs);
    assert!(hooked.status.success());
    assert_eq!(server.requests().len(), before);
    assert_eq!(entries(&yard).len(), 1);
}

#[test]
fn a_failed_flush_says_why_on_standard_error_without_json() {
    let yard = Yard::new();
    let failing = YardServer::failing(401);
    write(&yard, STAGE_BATCH, &connected(&failing.url()));
    let output = yard
        .scratch
        .command(Some(&yard.git), &yard.clock)
        .envs(connected(&failing.url()))
        .args(["yard", "flush"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let said = crate::common::stderr(&output);
    assert!(said.contains("401"), "{said}");
    assert!(said.contains("unauthenticated"), "{said}");
    assert_eq!(entries(&yard).len(), 1);
}

#[test]
fn a_flush_to_an_address_where_no_server_listens_fails_and_the_entries_stay() {
    let yard = Yard::new();
    let variables = connected(&YardServer::refusing());
    write(&yard, STAGE_BATCH, &variables);
    let (code, flushed) = flush(&yard, &variables);
    assert_eq!((code, &flushed["outcome"]), (Some(1), &json!("failed")));
    let message = flushed["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("did not answer"), "{message}");
    assert_eq!(entries(&yard).len(), 1);
}

#[test]
fn a_batch_that_does_not_go_ends_the_run_with_no_further_request() {
    let yard = Yard::new();
    let failing = YardServer::failing(503);
    let variables = connected(&failing.url());
    write(&yard, STAGE_BATCH, &[]);
    write(&yard, &crate::yard_connected::notes(17), &variables);
    let (code, flushed) = flush(&yard, &variables);
    assert_eq!((code, &flushed["outcome"]), (Some(1), &json!("failed")));
    assert_eq!(failing.requests().len(), 1);
    assert_eq!(flushed["remaining"], 17);
}
