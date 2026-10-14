//! `loft telemetry on|off|status` and `LOFT_TELEMETRY`: the two switches that turn telemetry
//! off, which of them decided, and the state each subcommand reports.

use std::fs;
use std::process::Output;

use crate::common::{CLOSED, EPOCH, Scratch, stdout};
use crate::schema;
use serde_json::{Value, json};
use software_plant::Clock;

const SCHEMA: &str = include_str!("../../../schemas/telemetry.json");

/// Runs `loft telemetry <args>` with `LOFT_TELEMETRY` set to `variable`, or unset.
fn telemetry(scratch: &Scratch, variable: Option<&str>, args: &[&str]) -> Output {
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    match variable {
        Some(value) => command.env("LOFT_TELEMETRY", value),
        None => command.env_remove("LOFT_TELEMETRY"),
    };
    command.arg("telemetry").args(args).output().unwrap()
}

/// The JSON document of `loft telemetry <action> --format json`, checked against its schema.
fn report(scratch: &Scratch, variable: Option<&str>, action: &str) -> Value {
    let output = telemetry(scratch, variable, &[action, "--format", "json"]);
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
fn telemetry_is_on_by_default() {
    let scratch = Scratch::empty();
    assert_eq!(state(&report(&scratch, None, "status")), ("on", "default"));
}

#[test]
fn off_is_recorded_and_status_reports_the_recorded_choice() {
    let scratch = Scratch::empty();
    assert_eq!(
        state(&report(&scratch, None, "off")),
        ("off", "loft telemetry off")
    );
    assert_eq!(
        state(&report(&scratch, None, "status")),
        ("off", "loft telemetry off")
    );
}

#[test]
fn on_after_off_reports_telemetry_on() {
    let scratch = Scratch::empty();
    report(&scratch, None, "off");
    assert_eq!(
        state(&report(&scratch, None, "on")),
        ("on", "loft telemetry on")
    );
}

#[test]
fn running_the_same_subcommand_twice_leaves_the_same_state() {
    let scratch = Scratch::empty();
    let once = report(&scratch, None, "off");
    let twice = report(&scratch, None, "off");
    assert_eq!(once, twice);
}

#[test]
fn the_variable_set_to_off_beats_a_recorded_on() {
    let scratch = Scratch::empty();
    report(&scratch, None, "on");
    assert_eq!(
        state(&report(&scratch, Some("off"), "status")),
        ("off", "LOFT_TELEMETRY")
    );
}

#[test]
fn any_value_of_the_variable_but_internal_switches_telemetry_off() {
    let scratch = Scratch::empty();
    for value in ["off", "0", "false", "no", "yes", "on"] {
        assert_eq!(
            state(&report(&scratch, Some(value), "status")),
            ("off", "LOFT_TELEMETRY"),
            "LOFT_TELEMETRY={value}"
        );
    }
}

#[test]
fn an_empty_variable_is_as_if_it_were_unset() {
    let scratch = Scratch::empty();
    assert_eq!(
        state(&report(&scratch, Some(""), "status")),
        ("on", "default")
    );
}

#[test]
fn internal_keeps_telemetry_on() {
    let scratch = Scratch::empty();
    assert_eq!(
        state(&report(&scratch, Some("internal"), "status")),
        ("on", "LOFT_TELEMETRY")
    );
}

#[test]
fn internal_sends_nothing_after_loft_telemetry_off() {
    let scratch = Scratch::empty();
    report(&scratch, None, "off");
    assert_eq!(
        state(&report(&scratch, Some("internal"), "status")),
        ("off", "loft telemetry off")
    );
}

#[test]
fn off_deletes_the_installation_token() {
    let scratch = Scratch::empty();
    fs::create_dir_all(scratch.config()).unwrap();
    let token = "8d2f61c3-5b0e-4a7c-9e14-2f6b3c8d9a01";
    let state = json!({ "installation": token, "noticeShown": true });
    fs::write(scratch.config().join("telemetry.json"), state.to_string()).unwrap();
    report(&scratch, None, "off");
    let kept = fs::read_to_string(scratch.config().join("telemetry.json")).unwrap();
    assert!(!kept.contains(token), "{kept}");
}

#[test]
fn status_names_the_collector_address() {
    let scratch = Scratch::empty();
    assert_eq!(report(&scratch, None, "status")["collector"], json!(CLOSED));
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    command.env_remove("LOFT_TELEMETRY_ENDPOINT");
    let output = command
        .args(["telemetry", "status", "--format", "json"])
        .output()
        .unwrap();
    let document = schema::conforming(&output.stdout, SCHEMA);
    assert_eq!(
        document["collector"],
        json!("https://askloft.ai/api/telemetry")
    );
}

#[test]
fn status_states_whether_the_notice_has_been_printed() {
    let scratch = Scratch::empty();
    assert_eq!(
        report(&scratch, None, "status")["noticeShown"],
        json!(false)
    );
    fs::create_dir_all(scratch.config()).unwrap();
    let state = json!({ "noticeShown": true });
    fs::write(scratch.config().join("telemetry.json"), state.to_string()).unwrap();
    assert_eq!(report(&scratch, None, "status")["noticeShown"], json!(true));
}

#[test]
fn status_in_text_names_the_state_the_switch_and_the_collector() {
    let scratch = Scratch::empty();
    let output = telemetry(&scratch, None, &["off"]);
    assert_eq!(output.status.code(), Some(0));
    let text = stdout(&telemetry(&scratch, None, &["status"]));
    assert!(
        text.contains("telemetry: off, by `loft telemetry off`"),
        "{text}"
    );
    assert!(text.contains(CLOSED), "{text}");
}

#[test]
fn a_settings_directory_that_cannot_be_written_is_an_error() {
    let scratch = Scratch::empty();
    let blocked = scratch.0.join("not-a-directory");
    fs::write(&blocked, "").unwrap();
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    let output = command
        .env("HOME", &blocked)
        .env("APPDATA", &blocked)
        .args(["telemetry", "off", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let document = schema::conforming(&output.stdout, SCHEMA);
    assert_eq!(document["outcome"], json!("error"));
    assert_eq!(document["errors"][0]["kind"], json!("settings"));
}

#[test]
fn switching_telemetry_off_leaves_the_update_check_as_it_was() {
    let scratch = Scratch::empty();
    report(&scratch, None, "off");
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    let output = command
        .env_remove("LOFT_UPDATE_CHECK")
        .args(["update-check", "status", "--format", "json"])
        .output()
        .unwrap();
    let document = schema::conforming(
        &output.stdout,
        include_str!("../../../schemas/update-check.json"),
    );
    assert_eq!(document["outcome"], json!("on"));
}
