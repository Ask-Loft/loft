//! `loft yard protocol` as a process: the protocol's version, `loft`'s own, and the formats it
//! reads and writes, the same inside and outside a repository, with nothing written; and the
//! record of each protocol version's fields, which a field removed or renamed from a yard
//! command's schema breaks until the protocol's version is raised.

use std::fs;

use serde_json::Value;
use software_plant::{Clock, GitScenario};

use crate::common::{EPOCH, Scratch, workspace};
use crate::snapshot::snapshot;
use crate::yard::document;
use crate::yard_commands::schema_fields;

fn protocol(scratch: &Scratch, git: Option<&GitScenario>) -> Value {
    let output = scratch.loft_with(
        &["yard", "protocol", "--format", "json"],
        git,
        &Clock::fixed(EPOCH),
    );
    assert_eq!(output.status.code(), Some(0));
    document("protocol", &output)
}

#[test]
fn the_report_gives_the_protocol_s_version_as_one_integer() {
    let report = protocol(&Scratch::empty(), None);
    assert!(
        report["protocol"]
            .as_u64()
            .is_some_and(|version| version >= 1)
    );
}

#[test]
fn the_report_gives_loft_s_own_version() {
    let report = protocol(&Scratch::empty(), None);
    assert_eq!(report["loft"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn the_report_lists_every_format_it_reads_and_names_the_one_it_writes() {
    let report = protocol(&Scratch::empty(), None);
    assert_eq!(report["formats"]["writes"], "yardlog/1");
    assert_eq!(report["formats"]["reads"], serde_json::json!(["yardlog/1"]));
}

#[test]
fn the_report_is_the_same_inside_and_outside_a_git_repository_and_writes_no_file() {
    let scratch = Scratch::empty();
    let before = snapshot(&scratch.0);
    let outside = protocol(&scratch, Some(&GitScenario::outside_repository()));
    let inside = protocol(&scratch, Some(&GitScenario::clean_on("main")));
    let without_git = protocol(&scratch, None);
    assert_eq!(outside, inside);
    assert_eq!(inside, without_git);
    assert!(snapshot(&scratch.0) == before, "a file changed");
    assert!(!scratch.home().join(".yard").exists());
}

/// The fields each version of the protocol promised, under `testdata/yard/protocol/`.
fn promised(version: u64) -> Vec<String> {
    let path = workspace().join(format!("testdata/yard/protocol/{version}.txt"));
    fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("no record of protocol {version}: {}", path.display()))
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The fields the protocol's version promised that the schemas no longer hold.
fn broken(promised: &[String], fields: &[String]) -> Vec<String> {
    promised
        .iter()
        .filter(|field| !fields.contains(field))
        .cloned()
        .collect()
}

fn current() -> Vec<String> {
    let version = protocol(&Scratch::empty(), None)["protocol"]
        .as_u64()
        .unwrap();
    promised(version)
}

#[test]
fn a_field_removed_or_renamed_from_a_yard_schema_needs_a_new_protocol_version() {
    let promised = current();
    let fields = schema_fields();
    assert_eq!(broken(&promised, &fields), Vec::<String>::new());
    let renamed: Vec<String> = fields
        .iter()
        .map(|field| field.replace("/properties/revision", "/properties/rev"))
        .collect();
    assert_ne!(broken(&promised, &renamed), Vec::<String>::new());
}

#[test]
fn a_field_added_to_a_yard_schema_leaves_the_protocol_s_version_as_it_was() {
    let promised = current();
    let mut fields = schema_fields();
    fields.push("yard-read/oneOf/0/properties/somethingNew".to_owned());
    assert_eq!(broken(&promised, &fields), Vec::<String>::new());
}
