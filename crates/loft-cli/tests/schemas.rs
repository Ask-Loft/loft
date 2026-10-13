//! Each command's JSON result is described by one schema under `schemas/`, named after the
//! command, that names its version.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// The subcommands `loft --help` lists.
fn subcommands() -> Vec<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_loft"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8(output.stdout).unwrap();
    help.lines()
        .skip_while(|line| !line.starts_with("Commands:"))
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help")
        .map(str::to_owned)
        .collect()
}

#[test]
fn each_command_has_one_schema_named_after_it_that_names_its_version() {
    let schemas = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let commands = subcommands();
    assert!(commands.len() >= 8, "{commands:?}");
    for command in commands {
        let path = schemas.join(format!("{command}.json"));
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("`loft {command}` has no schema {}", path.display()));
        let schema: Value = serde_json::from_str(&text).unwrap();
        let name = schema["properties"]["schema"]["const"].as_str().unwrap();
        let version = name.strip_prefix(&format!("loft-{command}/"));
        assert!(
            version.is_some_and(|version| version.parse::<u32>().is_ok()),
            "{command}: {name}"
        );
        assert_eq!(
            schema["$id"],
            format!("loft-{command}-{}.json", version.unwrap()),
            "{command}"
        );
    }
}
