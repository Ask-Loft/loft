//! The `trace` view: the document `loft reconcile` prints, with the branch's criteria, those
//! under each Feature whose file the branch added to or changed against the base revision, so
//! that the view grows with the change and not with the specification. Each Feature gives its
//! Requirements with their statements, and each criterion the key of its Requirement.

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
    let unverified = unverified(read.trace());
    let mut features = Vec::new();
    document.walk(&mut |node| {
        let Some(id) = node.id() else { return };
        let in_branch = sites
            .get(id.key())
            .is_some_and(|site| changed.contains(&site.path.to_string()));
        if node.kind() == NodeKind::Feature && in_branch {
            features.push(feature(document, node, &unverified));
        }
    });
    let outcome = reconcile::outcome(None, &reconciled);
    Ok(json!({
        "reconcile": read.json(&reconciled, &[], 0, outcome),
        "base": based.revision,
        "commit": based.commit,
        "features": features,
    }))
}

/// The reason each node linked `unverified` in the trace file gives, by its key.
fn unverified(trace: &Trace) -> BTreeMap<Key, String> {
    trace
        .links
        .iter()
        .filter_map(|link| match &link.evidence {
            Evidence::Unverified(reason) => Some((link.node, reason.clone())),
            Evidence::Tests(_) => None,
        })
        .collect()
}

/// A Feature with its Requirements, each by its key, label and statement, and its criteria, each
/// by its key, label and name, the key of its Requirement, and the reason one linked
/// `unverified` gives.
fn feature(document: &Document, node: Node<'_>, unverified: &BTreeMap<Key, String>) -> Value {
    let mut requirements = Vec::new();
    let mut criteria = Vec::new();
    node.walk(&mut |below| {
        if let Node::Requirement(requirement) = below {
            let entry = requirement_entry(document, below, &requirement.statement);
            criteria.extend(criteria_of(document, below, &entry["key"], unverified));
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
    unverified: &BTreeMap<Key, String>,
) -> Vec<Value> {
    let mut criteria = Vec::new();
    node.walk(&mut |below| {
        if below.kind() == NodeKind::Criterion {
            criteria.push(criterion_entry(document, below, requirement, unverified));
        }
    });
    criteria
}

/// A criterion by its key, label and name, the key of its Requirement, and the reason it is
/// linked `unverified`, where it is.
fn criterion_entry(
    document: &Document,
    node: Node<'_>,
    requirement: &Value,
    unverified: &BTreeMap<Key, String>,
) -> Value {
    let mut entry = named(document, node);
    if let Some(reason) = node.id().and_then(|id| unverified.get(&id.key())) {
        entry["unverified"] = json!(reason);
    }
    entry["requirement"] = requirement.clone();
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
