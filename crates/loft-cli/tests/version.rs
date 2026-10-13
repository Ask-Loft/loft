//! `loft --version` (F-46): the tool release with the format and the hashing algorithm it
//! implements, on three fixed lines or as one JSON document.

use crate::schema;

use std::process::{Command, Output};

fn loft(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_loft"))
        .args(args)
        .output()
        .unwrap()
}

fn lines() -> Vec<String> {
    let output = loft(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_first_line_is_loft_and_the_tool_version() {
    assert_eq!(lines()[0], format!("loft {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn the_second_line_is_the_format() {
    assert_eq!(lines()[1], "format loftspec/1");
}

#[test]
fn the_third_line_is_the_algorithm_and_there_is_no_other() {
    assert_eq!(lines()[2], "algorithm sha256-jcs/1");
    assert_eq!(lines().len(), 3);
}

#[test]
fn under_format_json_the_three_values_are_one_json_document() {
    let output = loft(&["--version", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let document = schema::conforming(
        &output.stdout,
        include_str!("../../../schemas/version.json"),
    );
    assert_eq!(document["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(document["format"], "loftspec/1");
    assert_eq!(document["algorithm"], "sha256-jcs/1");
}

#[test]
fn without_a_command_or_version_loft_still_exits_2() {
    assert_eq!(loft(&[]).status.code(), Some(2));
}

#[test]
fn help_still_exits_0() {
    assert_eq!(loft(&["--help"]).status.code(), Some(0));
}

#[test]
fn help_names_the_exit_codes_and_the_json_schemas() {
    let help = String::from_utf8(loft(&["--help"]).stdout).unwrap();
    for part in [
        "0 on success",
        "1 on an error",
        "2 on a usage error",
        "schemas/",
    ] {
        assert!(help.contains(part), "{part}: {help}");
    }
}
