//! Each command's JSON result is described by one schema under `schemas/`, named after the
//! command, that names its version; every schema describes sites and errors alike.

#[path = "common/help.rs"]
mod help;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn schemas() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas")
}

fn read(name: &str) -> Value {
    let path = schemas().join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|_| panic!("no schema {}", path.display()));
    serde_json::from_str(&text).unwrap()
}

#[test]
fn each_command_has_one_schema_named_after_it_that_names_its_version() {
    let commands = help::subcommands();
    assert!(commands.len() >= 8, "{commands:?}");
    for command in commands {
        let schema = read(&format!("{command}.json"));
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

#[test]
fn every_schema_describes_sites_and_errors_alike_but_for_the_error_kinds() {
    let reference = read("verify.json");
    let without_kinds = |defs: &Value| {
        let mut defs = defs.clone();
        defs["error"]["properties"]["kind"]["enum"] = Value::Null;
        defs
    };
    for entry in fs::read_dir(schemas()).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let defs = &read(&name)["$defs"];
        if defs.is_null() {
            continue;
        }
        assert_eq!(defs["site"], reference["$defs"]["site"], "{name}");
        assert_eq!(
            without_kinds(defs)["error"],
            without_kinds(&reference["$defs"])["error"],
            "{name}"
        );
    }
}
