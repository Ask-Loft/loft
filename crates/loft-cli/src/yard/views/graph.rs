//! What a `spec-change` view's diagram draws (the See a spec change Feature): the changed
//! Products, Feature Groups, Features, Requirements, criteria and Components as boxes, each box
//! above a changed one drawn for context, the changed decisions, and the `realizes` and subject
//! edges between them. A change to a node that has no box of its own, a constraint, a quality
//! attribute or a Section, marks the nearest box above it as changed. Beside the diagram, the
//! diff gives the words of every added, changed and removed node and decision, before and after.

use std::collections::{BTreeMap, BTreeSet};

use loftspec::decision::NodeRef;
use loftspec::diff::content_changes;
use loftspec::document::{Document, Edge};
use loftspec::identity::Key;
use loftspec::kind::NodeKind;
use loftspec::node::Node;
use serde_json::{Value, json};

use super::words;

/// One keyed node of a document: what a box or a tree line shows of it, and the node above it.
#[derive(Debug, Clone)]
struct Entry {
    kind: NodeKind,
    name: String,
    label: Option<String>,
    parent: Option<Key>,
    words: String,
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
                    words: words::of_node(node),
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
        NodeKind::Product
            | NodeKind::FeatureGroup
            | NodeKind::Feature
            | NodeKind::Requirement
            | NodeKind::Criterion
            | NodeKind::Component
    )
}

const CHANGES: [&str; 3] = ["added", "changed", "removed"];

/// The diagram of the change from `base` to `document`.
pub fn report(base: &Document, document: &Document) -> Value {
    let now = Walked::of(document);
    let then = Walked::of(base);
    let changes = content_changes(base, document);
    let mut marked: BTreeMap<Key, &'static str> = BTreeMap::new();
    let lists = [&changes.added, &changes.changed, &changes.removed];
    for (index, ids) in lists.into_iter().enumerate() {
        let walked = if index == 2 { &then } else { &now };
        for id in ids {
            mark(&mut marked, walked, id.key(), index);
        }
    }
    let shown = shown(&marked, &now, &then);
    let decisions = decisions(base, document);
    let decided = decisions.iter().map(|(_, value)| value.clone());
    json!({
        "nodes": nodes(&shown, &marked, &now, &then),
        "decisions": decided.clone().collect::<Vec<_>>(),
        "edges": edges(&shown, &decisions, document, base, &now, &then),
        "diff": diff(&lists, &now, &then).into_iter().chain(decided).collect::<Vec<_>>(),
    })
}

/// The label and name of each node and decision of `document` whose key is one of `keys`, by its
/// key, for a view to name them as the specification does.
pub fn named(document: &Document, keys: &BTreeSet<Key>) -> Value {
    let walked = Walked::of(document);
    let mut named = serde_json::Map::new();
    for (key, entry) in &walked.entries {
        if keys.contains(key) {
            named.insert(
                key.to_string(),
                json!({ "label": entry.label, "name": entry.name }),
            );
        }
    }
    for decision in &document.decisions {
        let key = decision.id.key();
        if keys.contains(&key) {
            let label = document
                .bindings
                .label_of(decision.id)
                .map(|l| l.to_string());
            named.insert(
                key.to_string(),
                json!({ "label": label, "name": decision.title }),
            );
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

/// Marks the box a change of the node `key` falls to: its own, or the nearest box above it,
/// changed.
fn mark(marked: &mut BTreeMap<Key, &'static str>, walked: &Walked, key: Key, index: usize) {
    let Some(entry) = walked.entries.get(&key) else {
        return;
    };
    if has_box(entry.kind) {
        marked.insert(key, CHANGES[index]);
    } else if let Some(boxed) = walked.boxed(key) {
        marked.entry(boxed).or_insert("changed");
    }
}

/// Every node added, changed and removed, with its words before and after, in the documents'
/// order: the added and changed in the tree's, then the removed in the base's.
fn diff(lists: &[&Vec<loftspec::ids::NodeId>; 3], now: &Walked, then: &Walked) -> Vec<Value> {
    let changed: BTreeMap<Key, usize> = lists
        .iter()
        .enumerate()
        .flat_map(|(index, ids)| ids.iter().map(move |id| (id.key(), index)))
        .collect();
    let removed = then.order.iter().filter(|key| changed.get(key) == Some(&2));
    let present = now.order.iter().filter(|key| changed.contains_key(key));
    present
        .chain(removed)
        .filter_map(|key| {
            let index = changed[key];
            let before = (index > 0).then(|| then.entries.get(key)).flatten();
            let after = (index < 2).then(|| now.entries.get(key)).flatten();
            let entry = after.or(before)?;
            Some(json!({
                "key": key.to_string(),
                "kind": entry.kind.as_str(),
                "label": entry.label,
                "name": entry.name,
                "change": CHANGES[index],
                "words": words::pair(
                    before.map(|entry| entry.words.as_str()),
                    after.map(|entry| entry.words.as_str()),
                ),
            }))
        })
        .collect()
}

/// The boxes drawn: each marked one, and each box above it, in the documents' order.
fn shown(marked: &BTreeMap<Key, &'static str>, now: &Walked, then: &Walked) -> Vec<Key> {
    let mut wanted = BTreeSet::new();
    for (key, change) in marked {
        let walked = if *change == "removed" { then } else { now };
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

fn nodes(
    shown: &[Key],
    marked: &BTreeMap<Key, &'static str>,
    now: &Walked,
    then: &Walked,
) -> Value {
    let nodes = shown.iter().filter_map(|key| {
        let change = marked.get(key).copied();
        let entry = if change == Some("removed") {
            then.entries.get(key)
        } else {
            now.entries.get(key).or_else(|| then.entries.get(key))
        }?;
        Some(json!({
            "key": key.to_string(),
            "kind": entry.kind.as_str(),
            "label": entry.label,
            "name": entry.name,
            "parent": entry.parent.and_then(|parent| boxed_parent(parent, now, then)),
            "change": change,
        }))
    });
    Value::Array(nodes.collect())
}

/// The key of the box above `parent`, by the document that holds it.
fn boxed_parent(parent: Key, now: &Walked, then: &Walked) -> Option<String> {
    now.boxed(parent)
        .or_else(|| then.boxed(parent))
        .map(|key| key.to_string())
}

/// Each decision added, changed or removed, by its key, with its words before and after.
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
        let was = before.get(key).map(|was| words::of_decision(was));
        let words = words::pair(was.as_deref(), Some(&words::of_decision(decision)));
        found.push((
            *key,
            decision_json(*key, label.as_deref(), &decision.title, change, &words),
        ));
    }
    for (key, decision) in &before {
        if !after.contains_key(key) {
            let label = base.bindings.label_of(decision.id).map(|l| l.to_string());
            let words = words::pair(Some(&words::of_decision(decision)), None);
            found.push((
                *key,
                decision_json(*key, label.as_deref(), &decision.title, "removed", &words),
            ));
        }
    }
    found
}

fn decision_json(key: Key, label: Option<&str>, title: &str, change: &str, words: &Value) -> Value {
    json!({
        "key": key.to_string(),
        "kind": "decision",
        "label": label,
        "name": title,
        "change": change,
        "words": words,
    })
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
