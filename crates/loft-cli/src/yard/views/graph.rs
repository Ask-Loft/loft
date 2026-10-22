//! What a `spec-change` view's diagram draws (the See a spec change Feature): the changed
//! Products, Feature Groups, Features and Components as boxes, each Feature Group and Product
//! above a changed box drawn for context, the changed decisions, and the `realizes` and subject
//! edges between them. A change to a node that has no box of its own, a Requirement, a
//! criterion, a constraint, a quality attribute or a Section, marks the nearest box above it as
//! changed, and a Feature counts its Requirements and criteria and the ones added, changed and
//! removed.

use std::collections::{BTreeMap, BTreeSet};

use loftspec::decision::NodeRef;
use loftspec::diff::content_changes;
use loftspec::document::{Document, Edge};
use loftspec::identity::Key;
use loftspec::kind::NodeKind;
use loftspec::node::Node;
use serde_json::{Value, json};

/// One keyed node of a document: what a box or a tree line shows of it, and the node above it.
#[derive(Debug, Clone)]
struct Entry {
    kind: NodeKind,
    name: String,
    label: Option<String>,
    parent: Option<Key>,
    requirements: usize,
    criteria: usize,
}

/// Each keyed node of a document, by its key, in the document's order.
struct Walked {
    entries: BTreeMap<Key, Entry>,
    order: Vec<Key>,
}

impl Walked {
    fn of(document: &Document) -> Self {
        let mut walked = Self {
            entries: BTreeMap::new(),
            order: Vec::new(),
        };
        for root in document.root_nodes() {
            walked.visit(document, root, None);
        }
        walked
    }

    fn visit(&mut self, document: &Document, node: Node<'_>, parent: Option<Key>) {
        let key = node.id().map(loftspec::ids::NodeId::key);
        if let (Some(key), Some(id)) = (key, node.id()) {
            let (requirements, criteria) = match node {
                Node::Feature(feature) => (
                    feature.requirements.len(),
                    feature
                        .requirements
                        .iter()
                        .map(|requirement| requirement.criteria.len())
                        .sum(),
                ),
                _ => (0, 0),
            };
            self.entries.insert(
                key,
                Entry {
                    kind: node.kind(),
                    name: node.name().into_owned(),
                    label: document
                        .bindings
                        .label_of(id)
                        .map(|label| label.to_string()),
                    parent,
                    requirements,
                    criteria,
                },
            );
            self.order.push(key);
        }
        for child in node.children() {
            self.visit(document, child, key.or(parent));
        }
    }

    /// The nearest box at or above `key`.
    fn boxed(&self, mut key: Key) -> Option<Key> {
        loop {
            let entry = self.entries.get(&key)?;
            if has_box(entry.kind) {
                return Some(key);
            }
            key = entry.parent?;
        }
    }
}

/// Whether nodes of `kind` are drawn as boxes of their own.
const fn has_box(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Product | NodeKind::FeatureGroup | NodeKind::Feature | NodeKind::Component
    )
}

/// How a box changed: its own change, and the Requirements and criteria beneath it.
#[derive(Debug, Default)]
struct Marked {
    change: Option<&'static str>,
    requirements: [usize; 3],
    criteria: [usize; 3],
}

const CHANGES: [&str; 3] = ["added", "changed", "removed"];

/// The diagram of the change from `base` to `document`.
pub fn report(base: &Document, document: &Document) -> Value {
    let now = Walked::of(document);
    let then = Walked::of(base);
    let changes = content_changes(base, document);
    let mut marked: BTreeMap<Key, Marked> = BTreeMap::new();
    let lists = [&changes.added, &changes.changed, &changes.removed];
    for (index, ids) in lists.into_iter().enumerate() {
        let walked = if index == 2 { &then } else { &now };
        for id in ids {
            mark(&mut marked, walked, id.key(), index);
        }
    }
    let shown = shown(&marked, &now, &then);
    let decisions = decisions(base, document);
    json!({
        "nodes": nodes(&shown, &marked, &now, &then),
        "decisions": decisions.iter().map(|(_, value)| value.clone()).collect::<Vec<_>>(),
        "edges": edges(&shown, &decisions, document, base, &now, &then),
    })
}

/// The label and name of each node and decision of `document` that `check`'s findings name, by
/// its key, for a view to name them as the specification does.
pub fn named_by_findings(document: &Document, check: &Value) -> Value {
    let keys: BTreeSet<&str> = ["findings", "resolved"]
        .iter()
        .filter_map(|list| check[list].as_array())
        .flatten()
        .filter_map(|finding| finding["subjects"].as_array())
        .flatten()
        .filter_map(|subject| subject["key"].as_str())
        .collect();
    let walked = Walked::of(document);
    let mut named = serde_json::Map::new();
    for (key, entry) in &walked.entries {
        let shown = key.to_string();
        if keys.contains(shown.as_str()) {
            named.insert(shown, json!({ "label": entry.label, "name": entry.name }));
        }
    }
    for decision in &document.decisions {
        let shown = decision.id.key().to_string();
        if keys.contains(shown.as_str()) {
            let label = document
                .bindings
                .label_of(decision.id)
                .map(|l| l.to_string());
            named.insert(shown, json!({ "label": label, "name": decision.title }));
        }
    }
    Value::Object(named)
}

/// The number of nodes and decisions of `document` that carry a label, by kind.
pub fn labels(document: &Document) -> Value {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    document.walk(&mut |node| {
        if node
            .id()
            .is_some_and(|id| document.bindings.label_of(id).is_some())
        {
            *counts.entry(node.kind().as_str()).or_default() += 1;
        }
    });
    let decisions = document
        .decisions
        .iter()
        .filter(|decision| document.bindings.label_of(decision.id).is_some())
        .count();
    if decisions > 0 {
        counts.insert("decision", decisions);
    }
    json!(counts)
}

/// Marks the box a change of the node `key` falls to.
fn mark(marked: &mut BTreeMap<Key, Marked>, walked: &Walked, key: Key, index: usize) {
    let Some(entry) = walked.entries.get(&key) else {
        return;
    };
    if has_box(entry.kind) {
        marked.entry(key).or_default().change = Some(CHANGES[index]);
        return;
    }
    let Some(boxed) = walked.boxed(key) else {
        return;
    };
    let mark = marked.entry(boxed).or_default();
    match entry.kind {
        NodeKind::Requirement => mark.requirements[index] += 1,
        NodeKind::Criterion => mark.criteria[index] += 1,
        _ => {}
    }
    mark.change.get_or_insert("changed");
}

/// The boxes drawn: each marked one, and each box above it, in the documents' order.
fn shown(marked: &BTreeMap<Key, Marked>, now: &Walked, then: &Walked) -> Vec<Key> {
    let mut wanted = BTreeSet::new();
    for (key, mark) in marked {
        let walked = if mark.change == Some("removed") {
            then
        } else {
            now
        };
        let mut at = Some(*key);
        while let Some(key) = at {
            let Some(entry) = walked.entries.get(&key) else {
                break;
            };
            if has_box(entry.kind) {
                wanted.insert(key);
            }
            at = entry.parent;
        }
    }
    let mut order: Vec<Key> = now.order.clone();
    order.extend(
        then.order
            .iter()
            .filter(|key| !now.entries.contains_key(key)),
    );
    order
        .into_iter()
        .filter(|key| wanted.contains(key))
        .collect()
}

fn nodes(shown: &[Key], marked: &BTreeMap<Key, Marked>, now: &Walked, then: &Walked) -> Value {
    let nodes = shown.iter().filter_map(|key| {
        let mark = marked.get(key);
        let change = mark.and_then(|mark| mark.change);
        let entry = if change == Some("removed") {
            then.entries.get(key)
        } else {
            now.entries.get(key).or_else(|| then.entries.get(key))
        }?;
        let mut node = json!({
            "key": key.to_string(),
            "kind": entry.kind.as_str(),
            "label": entry.label,
            "name": entry.name,
            "parent": entry.parent.and_then(|parent| boxed_parent(parent, now, then)),
            "change": change,
        });
        if entry.kind == NodeKind::Feature {
            node["requirements"] = json!(entry.requirements);
            node["criteria"] = json!(entry.criteria);
            let counts = |counts: [usize; 3]| {
                json!({ "added": counts[0], "changed": counts[1], "removed": counts[2] })
            };
            let (requirements, criteria) = mark.map_or(([0; 3], [0; 3]), |mark| {
                (mark.requirements, mark.criteria)
            });
            node["delta"] = json!({
                "requirements": counts(requirements),
                "criteria": counts(criteria),
            });
        }
        Some(node)
    });
    Value::Array(nodes.collect())
}

/// The key of the box above `parent`, by the document that holds it.
fn boxed_parent(parent: Key, now: &Walked, then: &Walked) -> Option<String> {
    now.boxed(parent)
        .or_else(|| then.boxed(parent))
        .map(|key| key.to_string())
}

/// Each decision added, changed or removed, by its key.
fn decisions(base: &Document, document: &Document) -> Vec<(Key, Value)> {
    let before: BTreeMap<Key, _> = base.decisions.iter().map(|d| (d.id.key(), d)).collect();
    let after: BTreeMap<Key, _> = document.decisions.iter().map(|d| (d.id.key(), d)).collect();
    let mut found = Vec::new();
    for (key, decision) in &after {
        let change = match before.get(key) {
            None => "added",
            Some(was) if was != decision => "changed",
            Some(_) => continue,
        };
        let label = document
            .bindings
            .label_of(decision.id)
            .map(|l| l.to_string());
        found.push((
            *key,
            decision_json(*key, label.as_deref(), &decision.title, change),
        ));
    }
    for (key, decision) in &before {
        if !after.contains_key(key) {
            let label = base.bindings.label_of(decision.id).map(|l| l.to_string());
            found.push((
                *key,
                decision_json(*key, label.as_deref(), &decision.title, "removed"),
            ));
        }
    }
    found
}

fn decision_json(key: Key, label: Option<&str>, title: &str, change: &str) -> Value {
    json!({ "key": key.to_string(), "kind": "decision", "label": label, "name": title, "change": change })
}

/// The `realizes` edges between boxes drawn, and the subject edges from each decision drawn to
/// the box of each node it names, where that box is drawn.
fn edges(
    shown: &[Key],
    decisions: &[(Key, Value)],
    document: &Document,
    base: &Document,
    now: &Walked,
    then: &Walked,
) -> Value {
    let drawn: BTreeSet<Key> = shown.iter().copied().collect();
    let named: BTreeSet<Key> = decisions.iter().map(|(key, _)| *key).collect();
    let mut found = BTreeSet::new();
    for (edges, walked) in [(&document.edges, now), (&base.edges, then)] {
        found.extend(
            edges
                .iter()
                .filter_map(|edge| drawn_edge(edge, &drawn, &named, walked)),
        );
    }
    let edges = found.into_iter().map(|(from, to, kind, role)| {
        let mut edge = json!({ "from": from, "to": to, "kind": kind });
        if let Some(role) = role {
            edge["role"] = json!(role);
        }
        edge
    });
    Value::Array(edges.collect())
}

/// An edge as the diagram draws it: from, to, its kind and a subject edge's role.
type Drawn = (String, String, &'static str, Option<&'static str>);

/// `edge` as the diagram draws it, where both its ends are drawn: a `realizes` edge between two
/// boxes, or a subject edge from a decision drawn to the box of the node it names.
fn drawn_edge(
    edge: &Edge,
    drawn: &BTreeSet<Key>,
    named: &BTreeSet<Key>,
    walked: &Walked,
) -> Option<Drawn> {
    match *edge {
        Edge::Realizes { feature, component } => {
            let (from, to) = (feature.key(), component.key());
            (drawn.contains(&from) && drawn.contains(&to))
                .then(|| (from.to_string(), to.to_string(), "realizes", None))
        }
        Edge::Subject {
            decision,
            subject: NodeRef::Held(node),
            role,
        } if named.contains(&decision.key()) => {
            let to = walked.boxed(node.key()).filter(|to| drawn.contains(to))?;
            let from = decision.key().to_string();
            Some((from, to.to_string(), "subject", Some(role.as_str())))
        }
        _ => None,
    }
}
