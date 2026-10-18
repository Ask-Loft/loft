//! Each command's JSON result is described by one schema under `schemas/`, named after the
//! command in the command table, that names its version and allows the envelope's result and error shapes; every
//! schema describes sites and errors alike. Every object a schema describes is closed, so a field
//! added to a command's output fails its schema until the schema names it, and every outcome is
//! named in words.

use loftspec::lint::RuleId;
use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::COMMANDS;
use crate::help;
use serde_json::{Value, json};

fn schemas() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas")
}

fn read(name: &str) -> Value {
    let path = schemas().join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|_| panic!("no schema {}", path.display()));
    serde_json::from_str(&text).unwrap()
}

/// The shapes a command's schema allows: its result shapes, then its error shape.
fn branches(schema: &Value) -> &[Value] {
    schema["oneOf"]
        .as_array()
        .expect("a result and an error shape")
}

#[test]
fn every_subcommand_has_an_entry_in_the_commands_and_no_entry_is_unknown() {
    let mut listed = help::subcommands();
    listed.sort();
    let mut entries: Vec<&str> = COMMANDS.iter().map(|(_, args, _, _)| args[0]).collect();
    entries.sort_unstable();
    entries.dedup();
    assert_eq!(listed, entries);
}

#[test]
fn every_import_source_has_one_entry_in_the_commands() {
    let mut listed = help::import_sources();
    listed.sort();
    let mut entries: Vec<&str> = COMMANDS
        .iter()
        .filter(|(_, args, _, _)| args[0] == "import")
        .map(|(_, args, _, _)| args[1])
        .collect();
    entries.sort_unstable();
    assert_eq!(listed, entries);
}

#[test]
fn each_command_has_one_schema_named_after_it_that_names_its_version() {
    for (_, _, _, command) in COMMANDS {
        let schema = read(&format!("{command}.json"));
        let name = &branches(&schema)[0]["properties"]["schema"]["const"];
        for branch in branches(&schema) {
            assert_eq!(&branch["properties"]["schema"]["const"], name, "{command}");
        }
        let version = name
            .as_str()
            .and_then(|name| name.strip_prefix(&format!("loft-{command}/")));
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
fn every_document_carries_schema_outcome_and_errors_and_an_error_carries_only_those() {
    for (_, _, _, command) in COMMANDS {
        let schema = read(&format!("{command}.json"));
        let (error, results) = branches(&schema).split_last().unwrap();
        for result in results {
            let required = result["required"].as_array().unwrap();
            for field in ["schema", "outcome", "errors"] {
                assert!(required.contains(&json!(field)), "{command}: {field}");
            }
        }
        let name = &error["properties"]["schema"]["const"];
        let only_three = json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["schema", "outcome", "errors"],
            "properties": {
                "schema": { "const": name },
                "outcome": { "const": "error" },
                "errors": { "type": "array", "items": { "$ref": "#/$defs/error" } },
            },
        });
        assert_eq!(error, &only_three, "{command}");
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

#[test]
fn the_check_schema_lists_exactly_the_rules_the_linter_defines() {
    let schema = read("check.json");
    let listed: Vec<&str> = schema["$defs"]["finding"]["properties"]["rule"]["enum"]
        .as_array()
        .expect("the rules a finding names")
        .iter()
        .map(|rule| rule.as_str().unwrap())
        .collect();
    let defined: Vec<&str> = loftspec::lint::RuleId::ALL
        .iter()
        .filter(|rule| !RuleId::TERM_REPORT.contains(rule))
        .map(|rule| rule.as_str())
        .collect();
    assert_eq!(listed, defined);
}

#[test]
fn the_terms_schema_lists_exactly_the_rules_of_the_term_report() {
    let schema = read("terms.json");
    let listed: Vec<&str> = schema["$defs"]["group"]["properties"]["rule"]["enum"]
        .as_array()
        .expect("the rules a group names")
        .iter()
        .map(|rule| rule.as_str().unwrap())
        .collect();
    let defined: Vec<&str> = RuleId::TERM_REPORT
        .iter()
        .map(|rule| rule.as_str())
        .collect();
    assert_eq!(listed, defined);
}

#[test]
fn the_openspec_schemas_list_exactly_the_rules_import_and_render_define() {
    let listed = |name: &str| -> Vec<String> {
        read(name)["$defs"]["finding"]["properties"]["rule"]["enum"]
            .as_array()
            .expect("the rules a finding names")
            .iter()
            .map(|rule| rule.as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        listed("import-openspec.json"),
        loftspec::convert::openspec::Rule::NAMES
    );
    assert_eq!(
        listed("render.json"),
        loftspec::render::openspec::Rule::NAMES
    );
}

/// The name of every schema under `schemas/`.
fn every_schema() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(schemas())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    names.sort();
    names
}

/// The JSON pointer of each object `schema` describes that admits a property it does not name;
/// gives the number of objects it describes.
fn open_objects(schema: &Value, at: &str, found: &mut Vec<String>) -> usize {
    match schema {
        Value::Object(fields) => {
            let object =
                fields.get("type") == Some(&json!("object")) || fields.contains_key("properties");
            if object && fields.get("additionalProperties") != Some(&Value::Bool(false)) {
                found.push(at.to_owned());
            }
            let nested: usize = fields
                .iter()
                .map(|(name, field)| open_objects(field, &format!("{at}/{name}"), found))
                .sum();
            usize::from(object) + nested
        }
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(index, item)| open_objects(item, &format!("{at}/{index}"), found))
            .sum(),
        _ => 0,
    }
}

#[test]
fn every_object_a_schema_describes_admits_no_field_it_does_not_name() {
    let names = every_schema();
    assert!(names.len() >= 12, "the scan found {names:?}");
    let mut open = Vec::new();
    let objects: usize = names
        .iter()
        .map(|name| open_objects(&read(name), name, &mut open))
        .sum();
    assert!(objects > 30, "the scan found {objects} objects");
    assert_eq!(open, Vec::<String>::new());
}

#[test]
fn an_open_object_in_a_schema_is_found() {
    let mut open = Vec::new();
    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": { "nested": { "type": "object", "properties": {} } },
    });
    assert_eq!(open_objects(&schema, "x.json", &mut open), 2);
    assert_eq!(open, ["x.json/properties/nested"]);
}

/// Whether `text` is one or more lowercase words, separated by single spaces.
fn in_words(text: &str) -> bool {
    !text.is_empty()
        && text
            .split(' ')
            .all(|word| !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase()))
}

#[test]
fn every_outcome_a_schema_allows_is_named_in_words() {
    for (_, _, _, command) in COMMANDS {
        let schema = read(&format!("{command}.json"));
        for branch in branches(&schema) {
            let outcome = &branch["properties"]["outcome"];
            let named: Vec<&Value> = match (&outcome["const"], &outcome["enum"]) {
                (Value::Null, Value::Array(options)) => options.iter().collect(),
                (single, Value::Null) if !single.is_null() => vec![single],
                _ => panic!("{command}: `outcome` is neither a const nor an enum: {outcome}"),
            };
            assert!(!named.is_empty(), "{command}");
            for name in named {
                let name = name.as_str().unwrap_or_default();
                assert!(in_words(name), "{command}: `{name}`");
            }
        }
    }
    assert!(in_words("passed") && in_words("up to date"));
    assert!(!in_words("") && !in_words("Passed") && !in_words("exit_1") && !in_words("2"));
}
