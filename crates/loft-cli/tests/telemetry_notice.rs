//! The telemetry notice: printed to standard error before the first event leaves a machine,
//! once, and again while it cannot be recorded.

use std::fs;

use crate::common::telemetry::{command, events};
use crate::common::{EPOCH, Scratch, stderr, stdout};
use software_plant::{Clock, Collector};

#[test]
fn the_first_run_prints_the_notice_on_standard_error_and_still_sends() {
    let scratch = Scratch::with_reading_list();
    let collector = Collector::accepting();
    let first = command(&scratch, None, &collector)
        .arg("check")
        .output()
        .unwrap();
    let notice = stderr(&first);
    for needed in [
        collector.url().as_str(),
        "loft telemetry off",
        "LOFT_TELEMETRY=off",
        "30 days",
        "90 days",
        "network address",
        "https://github.com/Ask-Loft/loft#telemetry-and-the-update-check",
    ] {
        assert!(notice.contains(needed), "{needed} in {notice}");
    }
    assert!(!notice.contains("anonymous"), "{notice}");
    let prose = notice.split_whitespace().collect::<Vec<_>>().join(" ");
    for sentence in [
        "Events with tokens are kept for at most 30 days and aggregates without tokens for at \
         most 90 days.",
        "the collector also sees the network address each event comes from.",
    ] {
        assert!(prose.contains(sentence), "{sentence} in {prose}");
    }
    assert_eq!(events(&collector).len(), 1);
    let off = scratch
        .command(None, &Clock::fixed(EPOCH))
        .arg("check")
        .output()
        .unwrap();
    assert_eq!(stdout(&first), stdout(&off));
}

#[test]
fn a_later_run_prints_no_notice() {
    let scratch = Scratch::with_reading_list();
    let collector = Collector::accepting();
    command(&scratch, None, &collector)
        .arg("check")
        .output()
        .unwrap();
    let second = command(&scratch, None, &collector)
        .arg("check")
        .output()
        .unwrap();
    assert_eq!(stderr(&second), "");
    assert_eq!(events(&collector).len(), 2);
}

#[test]
fn a_notice_that_cannot_be_recorded_is_printed_again_and_the_event_still_sent() {
    let scratch = Scratch::with_reading_list();
    let blocked = scratch.0.join("not-a-directory");
    fs::write(&blocked, "").unwrap();
    let collector = Collector::accepting();
    for _ in 0..2 {
        let output = command(&scratch, None, &collector)
            .env("HOME", &blocked)
            .env("APPDATA", &blocked)
            .arg("check")
            .output()
            .unwrap();
        assert!(
            stderr(&output).contains("telemetry is on"),
            "{}",
            stderr(&output)
        );
        assert_eq!(output.status.code(), Some(0));
    }
    assert_eq!(events(&collector).len(), 2);
}

#[test]
fn the_notice_names_every_field_of_the_event() {
    let schema: serde_json::Value =
        serde_json::from_str(crate::common::telemetry::EVENT_SCHEMA).unwrap();
    let scratch = Scratch::with_reading_list();
    let collector = Collector::accepting();
    let output = command(&scratch, None, &collector)
        .arg("check")
        .output()
        .unwrap();
    let notice = stderr(&output)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let phrase = |field: &str| match field {
        "schema" => "its schema",
        "eventId" => "event id",
        "version" => "version",
        "os" => "operating system",
        "arch" => "architecture",
        "installedBy" => "how it was installed",
        "time" => "the hour",
        "installation" => "this installation",
        "project" => "this checkout",
        "branch" => "its branch",
        "mode" => "whether CI or an agent ran it",
        "agent" => "an agent",
        "command" => "the command",
        "outcome" => "outcome",
        "errorKinds" => "error kinds",
        "duration" => "how long it took",
        other => panic!("the notice does not say how it names `{other}`"),
    };
    for field in schema["properties"].as_object().unwrap().keys() {
        assert!(notice.contains(phrase(field)), "{field}: {notice}");
    }
}
