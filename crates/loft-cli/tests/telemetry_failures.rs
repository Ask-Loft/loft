//! Sending never gets in the way: a collector that refuses, fails or stays silent changes no
//! exit code, no output and nothing on standard error, adds at most the network client's time
//! limit, and leaves nothing for a later run; and while telemetry is off nothing is sent,
//! written or printed.

use std::process::Output;
use std::time::{Duration, Instant};

use crate::common::telemetry::{asking, seen, sending};
use crate::common::{EPOCH, Scratch, stderr, stdout};
use software_plant::{Clock, Collector};

/// Runs `loft check --fail-on hint` in the Reading List, sending to `address`.
fn run(scratch: &Scratch, address: &str) -> (Output, Duration) {
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    sending(&mut command, address);
    let started = Instant::now();
    let output = command
        .args(["check", "--fail-on", "hint"])
        .output()
        .unwrap();
    (output, started.elapsed())
}

#[test]
fn a_collector_that_refuses_fails_or_stays_silent_changes_nothing_a_person_sees() {
    let scratch = Scratch::with_reading_list();
    seen(&scratch);
    let accepting = Collector::accepting();
    let (received, _) = run(&scratch, &accepting.url());
    let failing = Collector::failing(503);
    let silent = Collector::silent();
    for address in [Collector::refusing(), failing.url(), silent.url()] {
        let (output, _) = run(&scratch, &address);
        assert_eq!(output.status.code(), received.status.code(), "{address}");
        assert_eq!(stdout(&output), stdout(&received), "{address}");
        assert_eq!(stderr(&output), "", "{address}");
    }
}

#[test]
fn a_failed_event_is_neither_sent_again_nor_kept_for_a_later_run() {
    let scratch = Scratch::with_reading_list();
    seen(&scratch);
    let failing = Collector::failing(500);
    run(&scratch, &failing.url());
    assert_eq!(failing.requests().len(), 1);
    let accepting = Collector::accepting();
    run(&scratch, &accepting.url());
    assert_eq!(accepting.requests().len(), 1);
}

#[test]
fn a_silent_collector_and_a_silent_release_feed_add_at_most_the_time_limit() {
    let scratch = Scratch::with_reading_list();
    seen(&scratch);
    let silent = Collector::silent();
    let feed = software_plant::ReleaseFeed::silent();
    let mut off = scratch.command(None, &Clock::fixed(EPOCH));
    let started = Instant::now();
    off.args(["check", "--fail-on", "hint"]).output().unwrap();
    let without = started.elapsed();
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    asking(sending(&mut command, &silent.url()), &feed.url());
    let started = Instant::now();
    command
        .args(["check", "--fail-on", "hint"])
        .output()
        .unwrap();
    let with = started.elapsed();
    assert_eq!(feed.requests().len(), 1);
    // The limit is 500 milliseconds; the margin absorbs a loaded test machine.
    assert!(
        with < without + Duration::from_millis(700),
        "{with:?} against {without:?}"
    );
    assert!(with >= Duration::from_millis(400), "{with:?}");
}

#[test]
fn while_telemetry_is_off_nothing_is_sent_written_or_printed() {
    for variable in [Some("off"), None] {
        let scratch = Scratch::with_reading_list();
        let collector = Collector::accepting();
        let mut command = scratch.command(None, &Clock::fixed(EPOCH));
        sending(&mut command, &collector.url());
        if let Some(value) = variable {
            command.env("LOFT_TELEMETRY", value);
        } else {
            let mut off = scratch.command(None, &Clock::fixed(EPOCH));
            let recorded = off.args(["telemetry", "off"]).output().unwrap();
            assert_eq!(recorded.status.code(), Some(0));
        }
        let output = command.arg("check").output().unwrap();
        assert!(collector.requests().is_empty(), "{variable:?}");
        assert_eq!(stderr(&output), "", "{variable:?}");
        assert!(!scratch.0.join(".loft").exists(), "{variable:?}");
        let kept = scratch.config().join("telemetry.json");
        if variable.is_some() {
            assert!(!kept.exists(), "{variable:?}");
        } else {
            let kept = std::fs::read_to_string(kept).unwrap();
            assert!(!kept.contains("installation"), "{kept}");
        }
    }
}
