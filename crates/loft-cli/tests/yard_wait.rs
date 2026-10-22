//! `loft yard wait` as a process: a wait returns once its submission is no longer open, with the
//! answers and outcomes of its blocks, or `pending` once its timeout passes; it holds the
//! submission's lease while it runs, so a second wait is refused at once, and the lease is free
//! once the wait ends however it ends; and an agent waits only on what it sent.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use software_plant::Clock;

use crate::common::EPOCH;
use crate::snapshot::snapshot;
use crate::yard::{ASK, BRANCH, Yard, document};

/// Sends [`ASK`] as session `session`'s main agent, and gives the submission's key.
pub fn send(yard: &Yard, session: &str) -> String {
    let written = yard.write(session, ASK, &["--send"]);
    written["submission"].as_str().unwrap().to_owned()
}

/// Waits on `submission` as session `session`'s main agent, with `extra` arguments.
pub fn wait(yard: &Yard, session: &str, submission: &str, extra: &[&str]) -> (Option<i32>, Value) {
    let mut args = vec!["wait", "--session", session, "--submission", submission];
    args.extend_from_slice(extra);
    let output = yard.run(&args);
    (output.status.code(), document("wait", &output))
}

/// The file in git's own directory that names the wait holding a submission's lease.
fn holder(yard: &Yard, submission: &str) -> PathBuf {
    yard.scratch
        .0
        .join(".git/yard/leases")
        .join(format!("{submission}.holder"))
}

/// Waits until a running wait has taken the submission's lease.
fn until_leased(yard: &Yard, submission: &str) {
    let started = Instant::now();
    while !holder(yard, submission).exists() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "no wait took the lease"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_wait_on_a_submission_already_answered_returns_at_once_with_its_answers() {
    let yard = Yard::new();
    let submission = send(&yard, "s1");
    yard.answer("s1", r#"[{"block": "q1", "labels": ["Top"]}]"#);
    let started = Instant::now();
    let (code, waited) = wait(&yard, "s1", &submission, &["--timeout", "30"]);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(code, Some(0));
    assert_eq!(waited["outcome"], "answered");
    assert_eq!(waited["revision"], 2);
    assert_eq!(waited["blocks"][0]["id"], "q1");
    assert_eq!(waited["blocks"][0]["answer"]["labels"], json!(["Top"]));
    assert_eq!(waited["takenUp"], false);
}

#[test]
fn a_wait_returns_within_two_seconds_of_the_answer_that_completes_its_submission() {
    let yard = Yard::new();
    let submission = send(&yard, "s1");
    let waiting = yard.spawn(&["wait", "--session", "s1", "--submission", &submission]);
    until_leased(&yard, &submission);
    yard.answer("s1", r#"[{"block": "q1", "labels": ["Bottom"]}]"#);
    let answered = Instant::now();
    let output = waiting.wait_with_output().unwrap();
    assert!(answered.elapsed() < Duration::from_secs(2));
    let waited = document("wait", &output);
    assert_eq!(waited["outcome"], "answered");
    assert_eq!(waited["blocks"][0]["answer"]["labels"], json!(["Bottom"]));
}

#[test]
fn a_wait_on_a_withdrawn_submission_returns_that_it_was_withdrawn() {
    let yard = Yard::new();
    let submission = send(&yard, "s1");
    yard.run(&["withdraw", "--session", "s1", "--submission", &submission]);
    let (code, waited) = wait(&yard, "s1", &submission, &[]);
    assert_eq!(code, Some(0));
    assert_eq!(waited["outcome"], "withdrawn");
}

#[test]
fn a_wait_on_a_submission_its_close_withdrew_returns_that_yard_log_is_closed() {
    let yard = Yard::new();
    let submission = send(&yard, "s1");
    yard.run(&["close", "--session", "s1"]);
    let (code, waited) = wait(&yard, "s1", &submission, &[]);
    assert_eq!(code, Some(0));
    assert_eq!(waited["outcome"], "closed");
    assert_eq!(waited["revision"], 2);
}

#[test]
fn a_wait_whose_timeout_passes_returns_pending_exits_0_and_leaves_the_submission_open() {
    let yard = Yard::new();
    let submission = send(&yard, "s1");
    let (code, waited) = wait(&yard, "s1", &submission, &["--timeout", "1"]);
    assert_eq!(code, Some(0));
    assert_eq!(waited["outcome"], "pending");
    let read = yard.read("s1", &[]);
    assert_eq!(
        read["submissions"][0]["open"][0]["key"],
        submission.as_str()
    );
    let (_, again) = wait(&yard, "s1", &submission, &["--timeout", "0"]);
    assert_eq!(again["outcome"], "pending");
}

#[test]
fn a_second_wait_on_a_submission_is_refused_at_once_naming_the_wait_that_holds_it() {
    let yard = Yard::new();
    let submission = send(&yard, "s1");
    let mut first = yard.spawn(&["wait", "--session", "s1", "--submission", &submission]);
    until_leased(&yard, &submission);
    let started = Instant::now();
    let (code, refused) = wait(&yard, "s1", &submission, &["--timeout", "30"]);
    assert!(started.elapsed() < Duration::from_secs(5));
    first.kill().unwrap();
    first.wait().unwrap();
    assert_eq!(code, Some(1));
    assert_eq!(refused["outcome"], "refused");
    assert_eq!(refused["errors"][0]["kind"], "leased");
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("session s1"), "{message}");
}

#[test]
fn the_lease_is_free_once_the_wait_that_held_it_is_killed() {
    let yard = Yard::new();
    let submission = send(&yard, "s1");
    let mut first = yard.spawn(&["wait", "--session", "s1", "--submission", &submission]);
    until_leased(&yard, &submission);
    first.kill().unwrap();
    first.wait().unwrap();
    let (code, waited) = wait(&yard, "s1", &submission, &["--timeout", "0"]);
    assert_eq!(code, Some(0));
    assert_eq!(waited["outcome"], "pending");
}

#[test]
fn the_lease_is_a_lock_in_git_s_own_directory_and_nothing_of_it_is_under_yard() {
    let yard = Yard::new();
    let submission = send(&yard, "s1");
    let before = snapshot(&yard.folder(BRANCH));
    wait(&yard, "s1", &submission, &["--timeout", "0"]);
    assert!(
        snapshot(&yard.folder(BRANCH)) == before,
        "a file under .yard/ changed"
    );
    let lease = yard.scratch.0.join(".git/yard/leases");
    assert!(lease.join(format!("{submission}.lease")).is_file());
}

#[test]
fn a_subagent_s_wait_on_a_submission_another_agent_sent_is_refused_naming_the_sender() {
    let yard = Yard::new();
    let submission = send(&yard, "s1");
    let (code, refused) = wait(
        &yard,
        "s1",
        &submission,
        &["--agent", "a7", "--agent-type", "Explore"],
    );
    assert_eq!(code, Some(1));
    assert_eq!(refused["errors"][0]["kind"], "not-sender");
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("session s1"), "{message}");
}

#[test]
fn the_main_agent_s_wait_on_a_submission_its_own_subagent_sent_is_refused() {
    let yard = Yard::new();
    let written = yard.write(
        "s1",
        ASK,
        &["--send", "--agent", "a7", "--agent-type", "Plan"],
    );
    let submission = written["submission"].as_str().unwrap();
    let (code, refused) = wait(&yard, "s1", submission, &[]);
    assert_eq!(code, Some(1));
    assert_eq!(refused["errors"][0]["kind"], "not-sender");
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("Plan a7 of session s1"), "{message}");
}

#[test]
fn a_wait_on_a_submission_yard_log_does_not_hold_is_refused_at_once_naming_it() {
    let yard = Yard::new();
    send(&yard, "s1");
    let unknown = "01a16502-f6d2-7666-adad-3403c0a92870";
    let started = Instant::now();
    let (code, refused) = wait(&yard, "s1", unknown, &["--timeout", "30"]);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(code, Some(1));
    assert_eq!(refused["errors"][0]["kind"], "no-submission");
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains(unknown), "{message}");
}

/// Yard Log in a scratch tree whose clock lets time pass only as each run waits, so a wait of
/// minutes ends in moments.
fn passing() -> Yard {
    let mut yard = Yard::new();
    yard.clock = Clock::fixed(EPOCH).passing();
    yard
}

#[test]
fn a_wait_with_no_timeout_returns_pending_after_540_seconds_where_the_submission_is_still_open() {
    let yard = passing();
    let submission = send(&yard, "s1");
    let started = Instant::now();
    let (code, waited) = wait(&yard, "s1", &submission, &[]);
    assert!(started.elapsed() < Duration::from_secs(15));
    assert_eq!(code, Some(0));
    assert_eq!(waited["outcome"], "pending");
    assert_eq!(waited["waited"], 540);
}

#[test]
fn a_timeout_of_more_than_540_seconds_is_taken_as_540_seconds() {
    let yard = passing();
    let submission = send(&yard, "s1");
    for timeout in ["541", "600", "86400"] {
        let (code, waited) = wait(&yard, "s1", &submission, &["--timeout", timeout]);
        assert_eq!(code, Some(0), "{timeout}");
        assert_eq!(waited["outcome"], "pending", "{timeout}");
        assert_eq!(waited["waited"], 540, "{timeout}");
    }
    let (_, shorter) = wait(&yard, "s1", &submission, &["--timeout", "90"]);
    assert_eq!(shorter["waited"], 90);
}
