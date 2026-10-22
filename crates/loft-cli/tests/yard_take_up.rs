//! Taking up a submission: the main agent of another session waits on an open submission once
//! the session that sent it has not been heard for 60 seconds, no session continues it and no
//! wait holds it; the take-up is an Act, and the submission then counts as sent by the session
//! that took it up. A session is heard each time it runs a command that names it.

use serde_json::json;
use software_plant::Clock;

use crate::common::EPOCH;
use crate::yard::{BRANCH, Yard};
use crate::yard_wait::{send, wait};

/// A Yard Log whose submission session `s1` sent at [`EPOCH`], and the submission's key, with the
/// clock `seconds` later for the commands after.
fn sent_then(seconds: u64) -> (Yard, String) {
    let mut yard = Yard::new();
    let submission = send(&yard, "s1");
    yard.clock = Clock::fixed(EPOCH + seconds);
    (yard, submission)
}

#[test]
fn a_session_is_heard_each_time_it_runs_a_command_that_names_it() {
    let (yard, _) = sent_then(0);
    assert_eq!(
        yard.home_file("sessions.json"),
        json!({ "s1": "2026-01-01T00:00:00Z" })
    );
}

#[test]
fn another_session_s_main_agent_takes_up_a_submission_whose_session_was_not_heard_for_60_seconds() {
    let (yard, submission) = sent_then(61);
    let (code, waited) = wait(&yard, "s2", &submission, &["--timeout", "0"]);
    assert_eq!(code, Some(0));
    assert_eq!(waited["outcome"], "pending");
    assert_eq!(waited["takenUp"], true);
    assert_eq!(waited["revision"], 2);
}

#[test]
fn a_take_up_is_recorded_as_an_act_of_the_session_that_took_it_up() {
    let (yard, submission) = sent_then(61);
    wait(&yard, "s2", &submission, &["--timeout", "0"]);
    let root = std::fs::read_to_string(yard.folder(BRANCH).join("yardlog.yaml")).unwrap();
    assert!(root.contains("what: taken-up"), "{root}");
    assert!(root.contains("session: \"s2\""), "{root}");
    assert!(root.contains("at: 2026-01-01T00:01:01Z"), "{root}");
}

#[test]
fn a_submission_taken_up_counts_from_then_on_as_sent_by_the_session_that_took_it_up() {
    let (yard, submission) = sent_then(61);
    wait(&yard, "s2", &submission, &["--timeout", "0"]);
    let read = yard.read("s2", &[]);
    assert_eq!(read["submissions"][0]["agent"]["session"], "s2");
    let (code, again) = wait(&yard, "s2", &submission, &["--timeout", "0"]);
    assert_eq!(code, Some(0));
    assert_eq!(again["takenUp"], false);
}

#[test]
fn a_submission_whose_session_was_heard_within_60_seconds_is_not_taken_up() {
    let (yard, submission) = sent_then(59);
    let (code, refused) = wait(&yard, "s2", &submission, &["--timeout", "0"]);
    assert_eq!(code, Some(1));
    assert_eq!(refused["errors"][0]["kind"], "heard");
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("session s1"), "{message}");
    let root = std::fs::read_to_string(yard.folder(BRANCH).join("yardlog.yaml")).unwrap();
    assert!(!root.contains("taken-up"), "{root}");
}

#[test]
fn a_submission_of_a_session_another_continues_is_not_taken_up() {
    let mut yard = Yard::new();
    let submission = send(&yard, "s1");
    yard.read("s3", &["--continues", "s1"]);
    yard.clock = Clock::fixed(EPOCH + 61);
    let (code, refused) = wait(&yard, "s2", &submission, &["--timeout", "0"]);
    assert_eq!(code, Some(1));
    assert_eq!(refused["errors"][0]["kind"], "continued");
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("s3"), "{message}");
}

#[test]
fn a_subagent_of_another_session_does_not_take_up_a_submission() {
    let (yard, submission) = sent_then(61);
    let (code, refused) = wait(
        &yard,
        "s2",
        &submission,
        &["--timeout", "0", "--agent", "a1", "--agent-type", "Explore"],
    );
    assert_eq!(code, Some(1));
    assert_eq!(refused["errors"][0]["kind"], "not-sender");
}

#[test]
fn two_sessions_on_one_branch_never_both_wait_on_one_submission() {
    let (yard, submission) = sent_then(61);
    let mut taking = yard.spawn(&["wait", "--session", "s2", "--submission", &submission]);
    let holder = yard
        .scratch
        .0
        .join(".git/yard/leases")
        .join(format!("{submission}.holder"));
    let started = std::time::Instant::now();
    while !holder.exists() {
        assert!(started.elapsed().as_secs() < 10, "no wait took the lease");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let (code, refused) = wait(&yard, "s3", &submission, &["--timeout", "0"]);
    taking.kill().unwrap();
    taking.wait().unwrap();
    assert_eq!(code, Some(1));
    assert_eq!(refused["outcome"], "refused");
}
