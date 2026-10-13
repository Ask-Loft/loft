//! The subcommands `loft --help` lists, which the tests that hold every command to a rule read.

use std::process::Command;

pub fn subcommands() -> Vec<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_loft"))
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
