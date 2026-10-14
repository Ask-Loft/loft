//! `loft record-install <way>`: the record of how an executable was installed, kept beside the
//! records of other executables, and an install that never fails because of it.

use std::fs;
use std::path::PathBuf;
use std::process::Output;

use crate::common::{EPOCH, Scratch, stderr, stdout};
use crate::schema;
use serde_json::{Value, json};
use software_plant::Clock;

const SCHEMA: &str = include_str!("../../../schemas/record-install.json");

fn record(scratch: &Scratch, args: &[&str]) -> Output {
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    command.arg("record-install").args(args).output().unwrap()
}

fn records(scratch: &Scratch) -> Value {
    scratch.setting("install.json")
}

/// The path of the binary under test, as `loft` names its own executable.
fn executable() -> PathBuf {
    fs::canonicalize(env!("CARGO_BIN_EXE_loft")).unwrap()
}

#[test]
fn the_record_holds_the_executable_s_path_the_way_and_the_version() {
    let scratch = Scratch::empty();
    let output = record(&scratch, &["release-binary"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        records(&scratch)["installs"],
        json!([{
            "path": executable(),
            "way": "release-binary",
            "version": env!("CARGO_PKG_VERSION"),
        }])
    );
}

#[test]
fn nothing_is_written_to_standard_output_without_format_json() {
    let scratch = Scratch::empty();
    let output = record(&scratch, &["cargo"]);
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "");
}

#[test]
fn a_second_record_for_the_same_path_replaces_the_first() {
    let scratch = Scratch::empty();
    record(&scratch, &["cargo"]);
    record(&scratch, &["npm"]);
    let installs = records(&scratch)["installs"].clone();
    assert_eq!(installs.as_array().unwrap().len(), 1, "{installs}");
    assert_eq!(installs[0]["way"], json!("npm"));
}

#[test]
fn records_for_other_paths_are_kept_unchanged() {
    let scratch = Scratch::empty();
    fs::create_dir_all(scratch.config()).unwrap();
    let other = json!({ "path": "/elsewhere/loft", "way": "cargo", "version": "2.0.0" });
    let kept = json!({ "installs": [other] });
    fs::write(scratch.config().join("install.json"), kept.to_string()).unwrap();
    record(&scratch, &["npm"]);
    let installs = records(&scratch)["installs"].clone();
    assert_eq!(installs[0], other);
    assert_eq!(installs[1]["way"], json!("npm"));
}

#[test]
fn a_way_outside_the_three_is_a_usage_error_naming_them_and_nothing_is_written() {
    let scratch = Scratch::empty();
    let output = record(&scratch, &["brew"]);
    assert_eq!(output.status.code(), Some(2));
    let message = stderr(&output);
    for way in ["npm", "cargo", "release-binary"] {
        assert!(message.contains(way), "{message}");
    }
    assert!(!scratch.config().join("install.json").exists());
}

#[test]
fn the_json_document_names_the_path_and_the_way_recorded() {
    let scratch = Scratch::empty();
    let output = record(&scratch, &["npm", "--format", "json"]);
    let document = schema::conforming(&output.stdout, SCHEMA);
    assert_eq!(document["outcome"], json!("recorded"));
    assert_eq!(document["path"], json!(executable()));
    assert_eq!(document["way"], json!("npm"));
}

#[test]
fn a_record_that_cannot_be_written_exits_0_and_names_the_path() {
    let scratch = Scratch::empty();
    let blocked = scratch.0.join("not-a-directory");
    fs::write(&blocked, "").unwrap();
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    let output = command
        .env("HOME", &blocked)
        .env("APPDATA", &blocked)
        .args(["record-install", "npm", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(
        stderr(&output).contains(blocked.to_str().unwrap()),
        "{}",
        stderr(&output)
    );
    let document = schema::conforming(&output.stdout, SCHEMA);
    assert_eq!(document["outcome"], json!("unwritten"));
    assert_eq!(document["errors"][0]["kind"], json!("settings"));
}
