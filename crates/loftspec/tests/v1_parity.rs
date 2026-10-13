//! Agreement with v1: on each public v1 export, loftspec's structural and language findings and
//! its statement forms equal the outputs a v1 implementation wrote beside the export, apart from
//! the entries of the divergence register; and each entry is well formed and decided by a node of
//! `spec/`.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::common::parity::{
    Facts, converted, register, unexplained, v1_forms, v1_lint, v2_forms, v2_lint,
};

struct Fixture {
    name: &'static str,
    state: &'static str,
    lint: &'static str,
    ears: &'static str,
}

macro_rules! fixture {
    ($name:literal) => {
        Fixture {
            name: $name,
            state: include_str!(concat!(
                "../../../testdata/v1/",
                $name,
                "/loft-authored-state.json"
            )),
            lint: include_str!(concat!("../../../testdata/v1/", $name, "/v1-lint.json")),
            ears: include_str!(concat!("../../../testdata/v1/", $name, "/v1-ears.json")),
        }
    };
}

const FIXTURES: [Fixture; 2] = [fixture!("reading-list"), fixture!("d11-sequence-3")];

#[test]
fn the_shared_rules_report_what_v1_reports_apart_from_the_register() {
    let register = register();
    for fixture in &FIXTURES {
        let document = converted(fixture.state);
        let (missing, extra) = unexplained(
            &v1_lint(fixture.lint),
            &v2_lint(&document),
            &Facts::of(&document),
            &register,
        );
        assert!(
            missing.is_empty() && extra.is_empty(),
            "{}: only v1 reports {missing:#?}\nonly loftspec reports {extra:#?}",
            fixture.name
        );
    }
}

#[test]
fn a_name_stored_as_slots_has_the_form_v1_parses_it_to() {
    for fixture in &FIXTURES {
        let v1 = v1_forms(fixture.ears);
        let v2 = v2_forms(&converted(fixture.state));
        assert_eq!(
            v1.keys().collect::<Vec<_>>(),
            v2.keys().collect::<Vec<_>>(),
            "{}: the same Requirements",
            fixture.name
        );
        let disagreements: Vec<_> = v2
            .iter()
            .filter(|(id, form)| form.is_some() && v1[*id] != **form)
            .collect();
        assert!(
            disagreements.is_empty(),
            "{}: {disagreements:?}",
            fixture.name
        );
    }
}

#[test]
fn a_name_v1_parses_is_stored_as_slots() {
    for fixture in &FIXTURES {
        let v1 = v1_forms(fixture.ears);
        let v2 = v2_forms(&converted(fixture.state));
        let text: Vec<_> = v1
            .iter()
            .filter(|(id, form)| form.is_some() && v2[*id].is_none())
            .collect();
        assert!(text.is_empty(), "{}: stored as text {text:?}", fixture.name);
    }
}

#[test]
fn each_register_entry_is_well_formed() {
    let register = register();
    let ids: BTreeSet<&str> = register.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids.len(), register.len(), "ids are unique");
    for entry in &register {
        assert!(
            ["lint", "ears"].contains(&entry.surface.as_str()),
            "{}",
            entry.id
        );
        assert!(
            [
                "subject-external",
                "code-span",
                "subjects-are-criteria",
                "rule"
            ]
            .contains(&entry.condition.as_str()),
            "{}: condition `{}`",
            entry.id,
            entry.condition
        );
        assert!(
            ["v1-only", "v2-only"].contains(&entry.direction.as_str()),
            "{}",
            entry.id
        );
        assert!(
            !entry.rule.is_empty() && !entry.decided_by.is_empty(),
            "{}",
            entry.id
        );
    }
}

#[test]
fn each_register_entry_is_decided_by_a_node_of_the_specification() {
    let spec = spec_text(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec"));
    for entry in register() {
        for reference in &entry.decided_by {
            let bound = spec.contains(&format!("label: {reference}\n"));
            let keyed = spec.contains(&format!("key: {reference}\n"));
            assert!(bound || keyed, "{}: `{reference}` names no node", entry.id);
        }
    }
}

/// Every file of the tree, concatenated, leaving out entries whose names begin with `.`.
fn spec_text(dir: &Path) -> String {
    let mut text = String::new();
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        let hidden = path.file_name().unwrap().to_str().unwrap().starts_with('.');
        if hidden {
            continue;
        }
        if path.is_dir() {
            text.push_str(&spec_text(&path));
        } else {
            text.push_str(&fs::read_to_string(&path).unwrap());
        }
    }
    text
}
