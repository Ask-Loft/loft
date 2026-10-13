//! Each command's JSON result is described by one schema under `schemas/`, named after the
//! command, that names its version; every schema describes sites and errors alike.

#[path = "common/commands.rs"]
mod commands;
#[path = "common/help.rs"]
mod help;

use std::fs;
use std::path::{Path, PathBuf};

use commands::COMMANDS;
use serde_json::Value;

fn schemas() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas")
}

fn read(name: &str) -> Value {
    let path = schemas().join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|_| panic!("no schema {}", path.display()));
    serde_json::from_str(&text).unwrap()
}

/// The `schema` const of each branch of a command's schema.
fn names(schema: &Value) -> Vec<&str> {
    let branches = schema["oneOf"]
        .as_array()
        .expect("a result and an error shape");
    let names = branches
        .iter()
        .map(|branch| branch["properties"]["schema"]["const"].as_str());
    names.map(|name| name.expect("a schema const")).collect()
}

#[test]
fn every_subcommand_has_one_entry_in_the_commands_and_no_entry_is_unknown() {
    let mut listed = help::subcommands();
    listed.sort();
    let mut entries: Vec<&str> = COMMANDS.iter().map(|(_, args, _)| args[0]).collect();
    entries.sort_unstable();
    assert_eq!(listed, entries);
}

#[test]
fn each_command_has_one_schema_named_after_it_that_names_its_version() {
    for (_, args, _) in COMMANDS {
        let command = args[0];
        let schema = read(&format!("{command}.json"));
        let names = names(&schema);
        assert!(
            names.iter().all(|name| *name == names[0]),
            "{command}: {names:?}"
        );
        let version = names[0].strip_prefix(&format!("loft-{command}/"));
        assert!(
            version.is_some_and(|version| version.parse::<u32>().is_ok()),
            "{command}: {}",
            names[0]
        );
        assert_eq!(
            schema["$id"],
            format!("loft-{command}-{}.json", version.unwrap()),
            "{command}"
        );
    }
}

#[test]
fn every_document_carries_schema_outcome_and_errors_and_an_error_carries_only_those() {
    for (_, args, _) in COMMANDS {
        let command = args[0];
        let schema = read(&format!("{command}.json"));
        let branches = schema["oneOf"].as_array().unwrap();
        for branch in branches {
            let required = branch["required"].as_array().unwrap();
            for field in ["schema", "outcome", "errors"] {
                assert!(required.contains(&Value::from(field)), "{command}: {field}");
            }
        }
        let error = branches.last().unwrap();
        assert_eq!(
            error["properties"]["outcome"]["const"], "error",
            "{command}"
        );
        let mut fields: Vec<&String> = error["properties"].as_object().unwrap().keys().collect();
        fields.sort();
        assert_eq!(fields, ["errors", "outcome", "schema"], "{command}");
        assert_eq!(error["additionalProperties"], false, "{command}");
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
