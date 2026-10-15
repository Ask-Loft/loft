//! The `OpenSpec` projection: the main specs of one `OpenSpec` directory per Product, rendered so
//! that tools and people who read `OpenSpec` can read the specification, with a render map that
//! names the node each rendered requirement and scenario comes from, and findings for what
//! `OpenSpec`'s validator would reject and what the projection leaves out. Like every projection
//! it is derived and deterministic, and no reader takes input from it.

mod ext;
mod findings;
mod scan;
mod spec;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

pub use self::findings::{Finding, Rule};
use super::Labels;
use crate::convert::openspec::layout::{SPEC_FILE, SPECS};
use crate::document::Document;
use crate::ids::FeatureId;
use crate::index::IndexErrors;
use crate::node::Node;
use crate::tree::{Feature, FeatureGroup};

/// The render map's file name in the directory rendered to.
pub const RENDER_MAP: &str = "loft-render-map.json";

/// The render map's schema.
const MAP_SCHEMA: &str = "loft-render-map/1";

/// The shortest Purpose, in characters once trimmed, that `OpenSpec` does not warn about.
const PURPOSE_LENGTH: usize = 50;

/// The longest requirement text, in characters, that `OpenSpec` does not warn about.
const REQUIREMENT_LENGTH: usize = 500;

/// What one document renders to in `OpenSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenspecProjection {
    /// Each directory the main specs are written to, one per Product, by its path in the
    /// directory rendered to.
    pub roots: Vec<String>,
    /// Each `spec.md`, by its path in the directory rendered to.
    pub files: BTreeMap<String, String>,
    /// The text of the render map, [`RENDER_MAP`].
    pub map: String,
    pub findings: Vec<Finding>,
}

/// Renders a document's `OpenSpec` projection: under `specs/` for a document with one Product,
/// and under `<product slug>/specs/` for each Product of a document with more.
///
/// # Errors
///
/// A document whose references do not resolve renders nothing.
pub fn openspec(document: &Document) -> Result<OpenspecProjection, IndexErrors> {
    document.index()?;
    let labels = Labels::of(document);
    let several = document.products.len() > 1;
    let mut roots = Vec::new();
    let mut files = BTreeMap::new();
    let mut entries = Vec::new();
    let mut findings = Vec::new();
    for product in &document.products {
        let root = if several {
            format!("{}/{SPECS}", product.slug.as_str())
        } else {
            SPECS.to_owned()
        };
        roots.push(root.clone());
        for group in &product.feature_groups {
            for feature in &group.features {
                let path = format!("{root}/{}/{SPEC_FILE}", capability(group, feature));
                let spec = spec::write(feature, &labels);
                findings.extend(feature_findings(feature, &spec, &path));
                let entry = |marked| map_entry(&path, marked, &labels);
                entries.extend(spec.marked.iter().map(entry));
                files.insert(path, spec.text);
            }
        }
    }
    findings.extend(left_out(document));
    let map = json!({ "schema": MAP_SCHEMA, "entries": entries });
    let mut map = serde_json::to_string_pretty(&map).expect("a render map serialises");
    map.push('\n');
    Ok(OpenspecProjection {
        roots,
        files,
        map,
        findings,
    })
}

/// The render map's entry for a rendered requirement or scenario heading of the file at `path`.
fn map_entry(path: &str, marked: &spec::Marked, labels: &Labels) -> Value {
    let mut entry = json!({
        "file": path,
        "line": marked.line,
        "kind": marked.mark.as_str(),
    });
    if let Some(element) = marked.element {
        entry["key"] = json!(element.key().to_string());
        if let Some(label) = labels.label(element) {
            entry["label"] = json!(label);
        }
    }
    entry
}

/// The path of a Feature's capability: its `ext.openspec.capability`, else its Feature Group's
/// slug and its own.
fn capability(group: &FeatureGroup, feature: &Feature) -> String {
    ext::capability(&feature.details.ext).map_or_else(
        || format!("{}/{}", group.slug.as_str(), feature.slug.as_str()),
        str::to_owned,
    )
}

/// The findings of one Feature's `spec.md` at `path`.
fn feature_findings(feature: &Feature, spec: &spec::Spec, path: &str) -> Vec<Finding> {
    let at = |rule, line, key, detail| Finding {
        rule,
        file: Some(path.to_owned()),
        line: Some(line),
        key,
        detail,
    };
    let mut findings = Vec::new();
    let purpose = feature
        .details
        .body
        .as_deref()
        .map_or(0, |body| body.trim().chars().count());
    if purpose < PURPOSE_LENGTH {
        let key = feature.id.map(FeatureId::key);
        let detail = Some(format!("{purpose} characters"));
        findings.push(at(Rule::ShortPurpose, spec.purpose_line, key, detail));
    }
    let mut names = BTreeSet::new();
    for written in &spec.requirements {
        let read = scan::read(&spec.lines[written.lines.clone()]);
        let at = |rule, detail| at(rule, written.line, written.key, detail);
        if !read.modal() {
            findings.push(at(Rule::NoModal, None));
        }
        if !read.scenario {
            findings.push(at(Rule::NoScenario, None));
        }
        if read.length() > REQUIREMENT_LENGTH {
            let detail = Some(format!("{} characters", read.length()));
            findings.push(at(Rule::LongRequirement, detail));
        }
        if !names.insert(written.name.as_str()) {
            findings.push(at(Rule::DuplicateName, Some(written.name.clone())));
        }
    }
    findings
}

/// One finding for each kind of content the projection leaves out that the document holds.
fn left_out(document: &Document) -> Vec<Finding> {
    let (mut components, mut constraints, mut qualities) = (0, 0, 0);
    document.walk(&mut |node| match node {
        Node::Component(_) => components += 1,
        Node::Constraint(_) => constraints += 1,
        Node::QualityAttribute(_) => qualities += 1,
        _ => {}
    });
    [
        ("Components", components),
        ("Constraints", constraints),
        ("Quality Attributes", qualities),
        ("Decisions", document.decisions.len()),
        ("edges", document.edges.len()),
        ("tags", document.tags.len()),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(kind, count)| Finding {
        rule: Rule::LeftOut,
        file: None,
        line: None,
        key: None,
        detail: Some(format!("{count} {kind}")),
    })
    .collect()
}
