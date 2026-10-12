//! The comparison with v1's reference outputs (Agreement with v1): findings in their comparison
//! form, the divergence register `testdata/v1/divergences.json`, and the findings that neither
//! agreement nor an entry of the register explains.

use std::collections::{BTreeMap, BTreeSet};

use loftspec::boundary::Boundaries;
use loftspec::codec::{self, Sites};
use loftspec::convert::v1::convert;
use loftspec::convert::v1::export::Export;
use loftspec::document::Document;
use loftspec::lint::{self, LintConfig};
use loftspec::node::Node;
use loftspec::statement::Statement;
use serde_json::Value;

pub const REGISTER: &str = include_str!("../../../../testdata/v1/divergences.json");

/// A finding as both sides are compared: its rule, its severity and its sorted subject UUIDs.
pub type Normal = (String, String, Vec<String>);

/// The export converted and read back from its text form.
pub fn converted(export: &str) -> Document {
    let conversion = convert(&Export::from_json(export).unwrap()).unwrap();
    codec::read(&codec::write(&conversion.document)).unwrap()
}

/// The document read back with every backtick removed from its text, so that each code span
/// reads as prose: what the `code-span` condition compares with.
pub fn without_backticks(document: &Document) -> Document {
    let files = codec::write(document)
        .into_iter()
        .map(|(path, bytes)| {
            (
                path,
                bytes.into_iter().filter(|byte| *byte != b'`').collect(),
            )
        })
        .collect();
    codec::read(&files).unwrap()
}

/// v1's findings as `v1-lint.json` records them.
pub fn v1_lint(reference: &str) -> BTreeSet<Normal> {
    let reference: Value = serde_json::from_str(reference).unwrap();
    assert_eq!(reference["format"], "loftspec-v1-lint/1");
    assert_eq!(reference["ruleset"], "loft-lint-1.4.0");
    reference["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            let subjects = finding["subjects"].as_array().unwrap();
            (
                finding["rule"].as_str().unwrap().to_owned(),
                finding["severity"].as_str().unwrap().to_owned(),
                subjects
                    .iter()
                    .map(|s| s.as_str().unwrap().to_owned())
                    .collect(),
            )
        })
        .collect()
}

/// loftspec's structural and language findings on the document, one per rule, severity and
/// subject set.
pub fn v2_lint(document: &Document) -> BTreeSet<Normal> {
    lint::lint(document, &Sites::default(), &LintConfig::default())
        .into_iter()
        .filter(|finding| !finding.rule.as_str().starts_with("V2-"))
        .map(|finding| {
            let mut subjects: Vec<String> = finding
                .subjects
                .iter()
                .map(|s| s.key.uuid().to_string())
                .collect();
            subjects.sort_unstable();
            (
                finding.rule.as_str().to_owned(),
                finding.severity.as_str().to_owned(),
                subjects,
            )
        })
        .collect()
}

/// The UUIDs of the Components whose boundary, written or taken on, is `external`.
pub fn external(document: &Document) -> BTreeSet<String> {
    let boundaries = Boundaries::of(document);
    let mut external = BTreeSet::new();
    document.walk(&mut |node| {
        if let Node::Component(component) = node
            && let Some(id) = component.id
            && boundaries.is_external(id)
        {
            external.insert(id.key().uuid().to_string());
        }
    });
    external
}

/// v1's form of each Requirement as `v1-ears.json` records it, by UUID.
pub fn v1_forms(reference: &str) -> BTreeMap<String, Option<String>> {
    let reference: Value = serde_json::from_str(reference).unwrap();
    assert_eq!(reference["format"], "loftspec-v1-ears/1");
    reference["requirements"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, form)| (id.clone(), form.as_str().map(str::to_owned)))
        .collect()
}

/// The form of each Requirement the document stores as slots, by UUID; `None` where it stores
/// text.
pub fn v2_forms(document: &Document) -> BTreeMap<String, Option<String>> {
    let mut forms = BTreeMap::new();
    document.walk(&mut |node| {
        if let Node::Requirement(requirement) = node
            && let Some(id) = requirement.id
        {
            let form = match &requirement.statement {
                Statement::Form(form) => Some(form.kind().as_str().to_owned()),
                Statement::Text(_) => None,
            };
            forms.insert(id.key().uuid().to_string(), form);
        }
    });
    forms
}

/// An entry of the divergence register.
#[derive(Debug)]
pub struct Divergence {
    pub id: String,
    pub surface: String,
    pub rule: String,
    pub condition: String,
    pub direction: String,
    pub decided_by: Vec<String>,
}

/// The register's entries.
pub fn register() -> Vec<Divergence> {
    let register: Value = serde_json::from_str(REGISTER).unwrap();
    assert_eq!(register["format"], "loftspec-v1-divergences/1");
    register["divergences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let text = |field: &str| entry[field].as_str().unwrap_or_default().to_owned();
            Divergence {
                id: text("id"),
                surface: text("surface"),
                rule: text("rule"),
                condition: text("condition"),
                direction: text("direction"),
                decided_by: entry["decidedBy"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|reference| reference.as_str().unwrap().to_owned())
                    .collect(),
            }
        })
        .collect()
}

/// What the conditions read besides the finding: the Components whose boundary is `external`,
/// and loftspec's findings with every code span read as prose.
pub struct Facts {
    pub external: BTreeSet<String>,
    pub prose: BTreeSet<Normal>,
}

impl Facts {
    pub fn of(document: &Document) -> Self {
        Self {
            external: external(document),
            prose: v2_lint(&without_backticks(document)),
        }
    }
}

/// The entry that explains a finding only one side reports.
fn explains<'a>(
    register: &'a [Divergence],
    direction: &str,
    finding: &Normal,
    facts: &Facts,
) -> Option<&'a Divergence> {
    register.iter().find(|entry| {
        entry.surface == "lint"
            && entry.direction == direction
            && entry.rule == finding.0
            && match entry.condition.as_str() {
                "rule" => true,
                "subject-external" => finding.2.iter().any(|s| facts.external.contains(s)),
                "code-span" => facts.prose.contains(finding),
                _ => false,
            }
    })
}

/// The findings only v1 reports and only loftspec reports that no entry explains.
pub fn unexplained(
    v1: &BTreeSet<Normal>,
    v2: &BTreeSet<Normal>,
    facts: &Facts,
    register: &[Divergence],
) -> (Vec<Normal>, Vec<Normal>) {
    let keep = |direction: &str, only: Vec<&Normal>| -> Vec<Normal> {
        only.into_iter()
            .filter(|finding| explains(register, direction, finding, facts).is_none())
            .cloned()
            .collect()
    };
    (
        keep("v1-only", v1.difference(v2).collect()),
        keep("v2-only", v2.difference(v1).collect()),
    )
}
