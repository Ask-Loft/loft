//! The subcommands `loft --help` and `loft import --help` list, which the tests that hold every
//! command to a rule read.

use std::process::Command;

pub fn subcommands() -> Vec<String> {
    listed(&[])
}

/// The sources `loft import` takes, each a subcommand of its own.
pub fn import_sources() -> Vec<String> {
    listed(&["import"])
}

fn listed(command: &[&str]) -> Vec<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_loft"))
        .args(command)
        .arg("--help")
        .output()
        .unwrap();
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .skip_while(|line| !line.starts_with("Commands:"))
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help")
        .map(str::to_owned)
        .collect()
}
