//! `loft update-check on|off|status` and `LOFT_UPDATE_CHECK`: the two switches that turn the
//! update check off, which of them decided, and the latest release the check holds.

use std::fs;
use std::process::Output;

use crate::common::{EPOCH, Scratch, stdout};
use crate::schema;
use serde_json::{Value, json};
use software_plant::Clock;

const SCHEMA: &str = include_str!("../../../schemas/update-check.json");

/// Runs `loft update-check <args>` with `LOFT_UPDATE_CHECK` set to `variable`, or unset.
fn update_check(scratch: &Scratch, variable: Option<&str>, args: &[&str]) -> Output {
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    match variable {
        Some(value) => command.env("LOFT_UPDATE_CHECK", value),
        None => command.env_remove("LOFT_UPDATE_CHECK"),
    };
    command.arg("update-check").args(args).output().unwrap()
}

fn report(scratch: &Scratch, variable: Option<&str>, action: &str) -> Value {
    let output = update_check(scratch, variable, &[action, "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    schema::conforming(&output.stdout, SCHEMA)
}

fn state(document: &Value) -> (&str, &str) {
    (
        document["outcome"].as_str().unwrap(),
        document["decidedBy"].as_str().unwrap(),
    )
}

#[test]
fn the_update_check_is_on_by_default() {
    let scratch = Scratch::empty();
    assert_eq!(state(&report(&scratch, None, "status")), ("on", "default"));
}

#[test]
fn off_is_recorded_until_on() {
    let scratch = Scratch::empty();
    assert_eq!(
        state(&report(&scratch, None, "off")),
        ("off", "loft update-check off")
    );
    assert_eq!(
        state(&report(&scratch, None, "status")),
        ("off", "loft update-check off")
    );
    assert_eq!(
        state(&report(&scratch, None, "on")),
        ("on", "loft update-check on")
    );
}

#[test]
fn running_the_same_subcommand_twice_leaves_the_same_state() {
    let scratch = Scratch::empty();
    assert_eq!(report(&scratch, None, "off"), report(&scratch, None, "off"));
}

#[test]
fn any_value_of_the_variable_switches_the_check_off() {
    let scratch = Scratch::empty();
    report(&scratch, None, "on");
    for value in ["off", "0", "1", "on", "internal"] {
        assert_eq!(
            state(&report(&scratch, Some(value), "status")),
            ("off", "LOFT_UPDATE_CHECK"),
            "LOFT_UPDATE_CHECK={value}"
        );
    }
}

#[test]
fn status_states_that_no_answer_is_held_when_none_is() {
    let scratch = Scratch::empty();
    let document = report(&scratch, None, "status");
    assert!(document.get("latest").is_none(), "{document}");
    let text = stdout(&update_check(&scratch, None, &["status"]));
    assert!(text.contains("no answer from GitHub is held"), "{text}");
}

#[test]
fn status_names_the_latest_release_held_and_when_it_was_received() {
    let scratch = Scratch::empty();
    fs::create_dir_all(scratch.config()).unwrap();
    let held = json!({
        "latest": { "version": "2.4.0", "page": "https://github.com/Ask-Loft/loft/releases/tag/v2.4.0" },
        "receivedAt": EPOCH,
    });
    fs::write(scratch.config().join("update-check.json"), held.to_string()).unwrap();
    let document = report(&scratch, None, "status");
    assert_eq!(
        document["latest"],
        json!({ "version": "2.4.0", "receivedAt": "2026-01-01T00:00:00Z" })
    );
}

#[test]
fn status_in_text_names_the_state_and_the_switch() {
    let scratch = Scratch::empty();
    let text = stdout(&update_check(&scratch, Some("off"), &["status"]));
    assert!(
        text.contains("update check: off, by `LOFT_UPDATE_CHECK`"),
        "{text}"
    );
}

#[test]
fn switching_the_update_check_off_leaves_telemetry_as_it_was() {
    let scratch = Scratch::empty();
    report(&scratch, None, "off");
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    let output = command
        .env_remove("LOFT_TELEMETRY")
        .args(["telemetry", "status", "--format", "json"])
        .output()
        .unwrap();
    let document = schema::conforming(
        &output.stdout,
        include_str!("../../../schemas/telemetry.json"),
    );
    assert_eq!(document["outcome"], json!("on"));
}
