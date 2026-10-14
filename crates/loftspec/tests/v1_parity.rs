//! Agreement with v1: on each public v1 export, loftspec's structural and language findings and
//! its statement forms equal the outputs a v1 implementation wrote beside the export, apart from
//! the entries of the divergence register; and each entry is well formed, decided by a node of
//! `spec/` and explains a finding on a checked export.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::common::parity::{
    Divergence, Facts, Normal, converted, explaining, register, unexplained, v1_forms, v1_lint,
    v2_forms, v2_lint,
};
use loftspec::convert::v1::convert;
use loftspec::convert::v1::export::Export;

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
fn every_register_entry_explains_a_finding_on_a_checked_export() {
    let register = register();
    let mut explained = BTreeSet::new();
    for fixture in &FIXTURES {
        let document = converted(fixture.state);
        explained.extend(explaining(
            &v1_lint(fixture.lint),
            &v2_lint(&document),
            &Facts::of(&document),
            &register,
        ));
    }
    let stale: Vec<&str> = register
        .iter()
        .map(|entry| entry.id.as_str())
        .filter(|id| !explained.contains(id))
        .collect();
    assert!(
        stale.is_empty(),
        "entries that explain no finding: {stale:?}"
    );
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
fn a_hyphenated_tag_v1_matches_in_spaced_prose_is_a_recorded_divergence() {
    let document = crate::common::with_hyphenated_tag("Every key follows the key shape rule.");
    let v2 = v2_lint(&document);
    let facts = Facts::of(&document);
    let v1: BTreeSet<_> = v2.union(&facts.hyphenated).cloned().collect();
    assert!(v1.len() > v2.len(), "v1 matches the spaced words");
    let (missing, extra) = unexplained(&v1, &v2, &facts, &register());
    assert!(
        missing.is_empty() && extra.is_empty(),
        "{missing:?} {extra:?}"
    );
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
                "hyphenated-tag",
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

#[test]
fn each_export_the_tests_convert_is_checked_against_v1s_outputs_beside_it() {
    let checked: BTreeSet<&str> = FIXTURES.iter().map(|fixture| fixture.name).collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/v1");
    let mut exports = Vec::new();
    for dir in fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
    {
        let export = dir.join("loft-authored-state.json");
        if !export.is_file() {
            continue;
        }
        let name = dir.file_name().unwrap().to_str().unwrap().to_owned();
        let read = Export::from_json(&fs::read_to_string(export).unwrap()).unwrap();
        // An export that holds no element names no document, and so converts to nothing.
        let converts = convert(&read).is_ok();
        assert_eq!(
            converts,
            checked.contains(name.as_str()),
            "{name} is checked"
        );
        for output in ["v1-lint.json", "v1-ears.json"] {
            assert_eq!(dir.join(output).is_file(), converts, "{name}/{output}");
        }
        exports.push(name);
    }
    assert!(
        checked
            .iter()
            .all(|name| exports.iter().any(|export| export == name))
    );
}

/// A comparison form of a finding of `rule` on `subjects`.
fn normal(rule: &str, subjects: &[&str]) -> Normal {
    let subjects = subjects
        .iter()
        .map(|subject| (*subject).to_owned())
        .collect();
    (rule.to_owned(), "warning".to_owned(), subjects)
}

/// Facts with the criteria `criteria` and nothing else.
fn facts(criteria: &[&str]) -> Facts {
    Facts {
        external: BTreeSet::new(),
        criteria: criteria.iter().map(|id| (*id).to_owned()).collect(),
        prose: BTreeSet::new(),
        hyphenated: BTreeSet::new(),
    }
}

/// The one entry of the register with `condition`, alone.
fn entry_with(condition: &str) -> Vec<Divergence> {
    let entries: Vec<Divergence> = register()
        .into_iter()
        .filter(|entry| entry.condition == condition)
        .collect();
    assert!(!entries.is_empty(), "{condition}");
    entries
}

/// The findings only v1 reports and only loftspec reports that the entries leave unexplained.
fn left(v1: &[Normal], v2: &[Normal], facts: &Facts, entries: &[Divergence]) -> Vec<Normal> {
    let v1: BTreeSet<Normal> = v1.iter().cloned().collect();
    let v2: BTreeSet<Normal> = v2.iter().cloned().collect();
    let (missing, extra) = unexplained(&v1, &v2, facts, entries);
    missing.into_iter().chain(extra).collect()
}

#[test]
fn code_span_explains_what_v1_reports_and_loftspec_reports_once_backticks_are_removed() {
    let mut document = crate::common::small_document();
    document.products[0].feature_groups[0].name = "The `identity` group".to_owned();
    let facts = Facts::of(&document);
    let v2 = v2_lint(&document);
    let in_prose: Vec<Normal> = facts.prose.difference(&v2).cloned().collect();
    assert!(!in_prose.is_empty(), "a code span hides a finding");
    let entries = entry_with("code-span");
    let v2: Vec<Normal> = v2.into_iter().collect();
    let mut v1 = v2.clone();
    v1.extend(in_prose.iter().cloned());
    assert_eq!(left(&v1, &v2, &facts, &entries), Vec::new());
    let elsewhere = (
        in_prose[0].0.clone(),
        in_prose[0].1.clone(),
        vec!["x".to_owned()],
    );
    v1.push(elsewhere.clone());
    assert_eq!(left(&v1, &v2, &facts, &entries), [elsewhere]);
    let reported_by_loftspec = left(&[], &in_prose, &facts, &entries);
    assert_eq!(reported_by_loftspec, in_prose, "a v1-only condition");
}

#[test]
fn subjects_are_criteria_explains_a_finding_only_when_every_subject_is_a_criterion() {
    let facts = facts(&["c1", "c2"]);
    let entries = entry_with("subjects-are-criteria");
    let rule = entries[0].rule.clone();
    let criteria = normal(&rule, &["c1", "c2"]);
    assert_eq!(left(&[criteria], &[], &facts, &entries), Vec::new());
    let mixed = normal(&rule, &["c1", "n1"]);
    assert_eq!(
        left(std::slice::from_ref(&mixed), &[], &facts, &entries),
        [mixed]
    );
}

#[test]
fn subjects_are_criteria_explains_no_finding_without_a_subject() {
    let facts = facts(&["c1"]);
    let entries = entry_with("subjects-are-criteria");
    let subjectless = normal(&entries[0].rule, &[]);
    assert_eq!(
        left(std::slice::from_ref(&subjectless), &[], &facts, &entries),
        [subjectless]
    );
}
