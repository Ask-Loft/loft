//! The `trace` view: the counts and outcome of the document `loft reconcile` prints, with the
//! branch's criteria, those under each Feature whose file the branch added to or changed against
//! the base revision, so that the view grows with the change and not with the specification.
//! Each Feature gives its Requirements with their statements, and each criterion the key of its
//! Requirement and its own tests and rules; reconcile's lists of tests and findings, which cover
//! the whole repository, are not recorded.

use std::collections::BTreeMap;
use std::path::Path;

use loftspec::document::Document;
use loftspec::identity::Key;
use loftspec::kind::NodeKind;
use loftspec::node::Node;
use loftspec::publish::{ReadTree, RepoState};
use loftspec::statement::Statement;
use loftspec::trace::{Evidence, Trace};
use loftspec_store::repository::GitCommand;
use serde_json::{Value, json};

use crate::opened::Opened;
use crate::stop::Stop;
use crate::{base, reconcile, status};

/// What a `trace` view is computed over: the tree under `root`, opened, the repository's
/// `state`, and the `base` revision.
pub struct Over<'a> {
    pub git: &'a GitCommand,
    pub root: &'a Path,
    pub opened: &'a Opened,
    pub state: RepoState,
    pub base: &'a str,
}

/// The report of a `trace` view: the tree is read once, for the reconciliation, the nodes
/// changed since the base and the Features that hold them.
pub fn report(over: &Over<'_>) -> Result<Value, Stop> {
    let tree = ReadTree::read(&over.opened.files)?;
    let (document, sites) = (tree.document(), tree.sites());
    let read = reconcile::Read::of(&over.opened.store, document, sites)?;
    let reconciled = read.reconciled();
    let state = over.state.clone();
    let mut status = status::Report::of(&over.opened.store, &tree, || Ok(state))?;
    let based = base::read_tree(over.git, over.root, over.base)?;
    status.compare(&based, &tree);
    let changed = status.changed_paths();
    let outcome = reconcile::outcome(None, &reconciled);
    let mut reconcile = read.json(&reconciled, &[], 0, outcome);
    let links = Links::of(read.trace(), &reconcile);
    // The lists of the whole repository: each of the branch's criteria carries its own share.
    if let Some(fields) = reconcile.as_object_mut() {
        fields.remove("tests");
        fields.remove("findings");
    }
    let mut features = Vec::new();
    document.walk(&mut |node| {
        let Some(id) = node.id() else { return };
        let in_branch = sites
            .get(id.key())
            .is_some_and(|site| changed.contains(&site.path.to_string()));
        if node.kind() == NodeKind::Feature && in_branch {
            features.push(feature(document, node, &links));
        }
    });
    Ok(json!({
        "reconcile": reconcile,
        "base": based.revision,
        "commit": based.commit,
        "features": features,
    }))
}

/// What each node's record carries of its links, by its key: the reason one linked `unverified`
/// gives, and its tests and the rules of its findings from the whole of what `loft reconcile`
/// reported, before the report's lists are shortened.
struct Links {
    unverified: BTreeMap<Key, String>,
    tests: BTreeMap<String, Vec<Value>>,
    rules: BTreeMap<String, Vec<Value>>,
}

impl Links {
    fn of(trace: &Trace, reconcile: &Value) -> Self {
        let unverified = trace
            .links
            .iter()
            .filter_map(|link| match &link.evidence {
                Evidence::Unverified(reason) => Some((link.node, reason.clone())),
                Evidence::Tests(_) => None,
            })
            .collect();
        let mut tests: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for test in entries(reconcile, "tests") {
            let mut test = test.clone();
            if let Some(Value::String(key)) =
                test.as_object_mut().and_then(|test| test.remove("key"))
            {
                tests.entry(key).or_default().push(test);
            }
        }
        let mut rules: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for finding in entries(reconcile, "findings") {
            if let Some(key) = finding["key"].as_str() {
                rules
                    .entry(key.to_owned())
                    .or_default()
                    .push(finding["rule"].clone());
            }
        }
        Self {
            unverified,
            tests,
            rules,
        }
    }
}

/// The entries of the list `name` of a reconcile document.
fn entries<'a>(reconcile: &'a Value, name: &str) -> &'a [Value] {
    reconcile[name].as_array().map_or(&[], Vec::as_slice)
}

/// A Feature with its Requirements, each by its key, label and statement, and its criteria, each
/// by its key, label and name, the key of its Requirement, and the reason one linked
/// `unverified` gives, its tests and the rules reported of it.
fn feature(document: &Document, node: Node<'_>, links: &Links) -> Value {
    let mut requirements = Vec::new();
    let mut criteria = Vec::new();
    node.walk(&mut |below| {
        if let Node::Requirement(requirement) = below {
            let entry = requirement_entry(document, below, &requirement.statement);
            criteria.extend(criteria_of(document, below, &entry["key"], links));
            requirements.push(entry);
        }
    });
    let mut feature = named(document, node);
    feature["requirements"] = Value::Array(requirements);
    feature["criteria"] = Value::Array(criteria);
    feature
}

/// A Requirement by its key, label and sentence, and, in an EARS form, its form and its slots.
fn requirement_entry(document: &Document, node: Node<'_>, statement: &Statement) -> Value {
    let mut entry = named(document, node);
    if let Statement::Form(form) = statement {
        let (condition, system, shall) = form.parts();
        entry["statement"] = json!({
            "form": form.kind().as_str(),
            "condition": condition,
            "system": system,
            "shall": shall,
        });
    }
    entry
}

/// The criteria of a Requirement, each with the key of its Requirement.
fn criteria_of(
    document: &Document,
    node: Node<'_>,
    requirement: &Value,
    links: &Links,
) -> Vec<Value> {
    let mut criteria = Vec::new();
    node.walk(&mut |below| {
        if below.kind() == NodeKind::Criterion {
            criteria.push(criterion_entry(document, below, requirement, links));
        }
    });
    criteria
}

/// A criterion by its key, label and name, the key of its Requirement, the reason it is linked
/// `unverified`, where it is, and its tests and the rules reported of it, each list empty where
/// there are none.
fn criterion_entry(
    document: &Document,
    node: Node<'_>,
    requirement: &Value,
    links: &Links,
) -> Value {
    let mut entry = named(document, node);
    if let Some(reason) = node.id().and_then(|id| links.unverified.get(&id.key())) {
        entry["unverified"] = json!(reason);
    }
    entry["requirement"] = requirement.clone();
    let key = entry["key"].as_str().unwrap_or_default().to_owned();
    entry["tests"] = Value::Array(links.tests.get(&key).cloned().unwrap_or_default());
    entry["rules"] = Value::Array(links.rules.get(&key).cloned().unwrap_or_default());
    entry
}

fn named(document: &Document, node: Node<'_>) -> Value {
    let id = node.id();
    json!({
        "key": id.map(|id| id.key().to_string()),
        "label": id.and_then(|id| document.bindings.label_of(id)).map(|label| label.to_string()),
        "name": node.name(),
    })
}
