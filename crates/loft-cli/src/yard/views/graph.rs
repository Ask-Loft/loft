//! What a `spec-change` view's diagram draws (the See a spec change Feature): every changed node
//! as a box of its own, whatever its kind, the changed decisions, each node a decision gained as a
//! subject, each box above any of them drawn for context, and the `realizes` and subject edges
//! between them. A decision whose subjects changed is changed, and draws its subject edges as its
//! own document holds them, so that a subject it lost has no edge. Beside the diagram, the diff
//! gives the words of every added, changed and removed node and decision, before and after, and
//! the subjects each decision gained and lost.

use std::collections::{BTreeMap, BTreeSet};

use loftspec::bindings::Bindings;
use loftspec::decision::NodeRef;
use loftspec::diff::{DecisionChanges, Subject, SubjectChange, content_changes, decision_changes};
use loftspec::document::{Document, Edge};
use loftspec::identity::Key;
use loftspec::ids::ElementId;
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
}

const CHANGES: [&str; 3] = ["added", "changed", "removed"];

/// The change of each key of the added, changed and removed lists, by key.
fn marks(lists: [&[impl Copy + Into<ElementId>]; 3]) -> BTreeMap<Key, &'static str> {
    lists
        .into_iter()
        .zip(CHANGES)
        .flat_map(|(ids, change)| ids.iter().map(move |id| ((*id).into().key(), change)))
        .collect()
}

/// A node as the tree holds it, or as the base held it where the tree no longer holds it.
fn entry<'a>(key: &Key, now: &'a Walked, then: &'a Walked) -> Option<&'a Entry> {
    now.entries.get(key).or_else(|| then.entries.get(key))
}

/// The diagram of the change from `base` to `document`.
pub fn report(base: &Document, document: &Document) -> Value {
    let now = Walked::of(document);
    let then = Walked::of(base);
    let changes = content_changes(base, document);
    let lists = [&changes.added, &changes.changed, &changes.removed];
    let marked = marks(lists.map(Vec::as_slice));
    let decided = decision_changes(base, document);
    let gained: BTreeSet<Key> = (decided.subjects.values())
        .flat_map(|change| &change.gained)
        .filter_map(|(subject, _)| match subject {
            NodeRef::Held(node) => Some(node.key()),
            NodeRef::Former(_) => None,
        })
        .collect();
    let shown = shown(&marked, &gained, &now, &then);
    let documents = Documents {
        base,
        document,
        now: &now,
        then: &then,
    };
    let decided = decisions(&decided, &documents);
    let edges = edges(&shown, &decided, &documents);
    // Only a decision's entry in the diff lists its subjects; its box needs none.
    let (boxes, listed): (Vec<Value>, Vec<Value>) = decided
        .into_iter()
        .map(|decision| {
            let mut listed = decision.entry.clone();
            if let Some(subjects) = decision.subjects {
                listed["subjects"] = subjects;
            }
            (decision.entry, listed)
        })
        .unzip();
    json!({
        "nodes": nodes(&shown, &marked, &now, &then),
        "decisions": boxes,
        "edges": edges,
        "diff": diff(&lists, &now, &then).into_iter().chain(listed).collect::<Vec<_>>(),
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

/// The boxes drawn: each marked one, each subject a decision gained, and each box above any of
/// them, as the tree holds it, or as the base held it where the tree no longer holds it, in the
/// documents' order.
fn shown(
    marked: &BTreeMap<Key, &'static str>,
    gained: &BTreeSet<Key>,
    now: &Walked,
    then: &Walked,
) -> Vec<Key> {
    let mut wanted = BTreeSet::new();
    for key in marked.keys().chain(gained) {
        // A removed node lies where the base held it, but each ancestor the tree still holds lies
        // where the tree holds it, so that a node that moved is drawn once, in its place.
        let mut at = Some(*key);
        while let Some(found) = at.and_then(|key| entry(&key, now, then)) {
            wanted.extend(at);
            at = found.parent;
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

/// Each box drawn, with its kind, label, name, the box above it and its change, where it has one.
fn nodes(
    shown: &[Key],
    marked: &BTreeMap<Key, &'static str>,
    now: &Walked,
    then: &Walked,
) -> Value {
    let nodes = shown.iter().filter_map(|key| {
        let change = marked.get(key).copied();
        let entry = entry(key, now, then)?;
        Some(json!({
            "key": key.to_string(),
            "kind": entry.kind.as_str(),
            "label": entry.label,
            "name": entry.name,
            "parent": entry.parent.map(|parent| parent.to_string()),
            "change": change,
        }))
    });
    Value::Array(nodes.collect())
}

/// A decision the diagram draws: its key, whether only the base holds it, its entry, and the
/// subjects it gained and lost, which only its entry in the diff lists.
struct Decided {
    key: Key,
    removed: bool,
    entry: Value,
    subjects: Option<Value>,
}

/// The two documents compared and their keyed nodes.
struct Documents<'a> {
    base: &'a Document,
    document: &'a Document,
    now: &'a Walked,
    then: &'a Walked,
}

/// Each decision `changes` holds, with its words before and after and the subjects it gained and
/// lost: the added and changed in the tree's order, then the removed in the base's.
fn decisions(changes: &DecisionChanges, documents: &Documents<'_>) -> Vec<Decided> {
    let Documents { base, document, .. } = *documents;
    let before: BTreeMap<Key, _> = base.decisions.iter().map(|d| (d.id.key(), d)).collect();
    let change = marks([&changes.added, &changes.changed, &changes.removed].map(Vec::as_slice));
    let in_tree = document.decisions.iter().map(|is| (is, document));
    let removed = (changes.removed.iter()).filter_map(|id| Some((*before.get(&id.key())?, base)));
    in_tree
        .chain(removed)
        .filter_map(|(decision, holder)| {
            let key = decision.id.key();
            let change = *change.get(&key)?;
            let was = before.get(&key).copied();
            let is = (change != "removed").then_some(decision);
            let label = holder.bindings.label_of(decision.id).map(|l| l.to_string());
            let words = words::pair(
                was.map(words::of_decision).as_deref(),
                is.map(words::of_decision).as_deref(),
            );
            Some(Decided {
                key,
                removed: is.is_none(),
                entry: decision_json(key, label.as_deref(), &decision.title, change, &words),
                subjects: changes
                    .subjects
                    .get(&decision.id)
                    .map(|change| listed(change, documents)),
            })
        })
        .collect()
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

/// The subjects `change` holds, as a decision's entry lists them: each gained, as the tree names
/// it, and each lost, as the base does.
fn listed(change: &SubjectChange, documents: &Documents<'_>) -> Value {
    let gained = (change.gained.iter()).map(|named| subject(named, "added", documents, false));
    let lost = (change.lost.iter()).map(|named| subject(named, "removed", documents, true));
    Value::Array(gained.chain(lost).collect())
}

/// A subject as a decision's entry lists it: the node's key, kind, label and name, as the tree
/// holds them, or as the base did for a subject `lost` or a node the tree no longer holds; or the
/// label its departed binding keeps where neither document holds it, or a former label no binding
/// gives; with its role and its change.
fn subject(
    (subject, role): &Subject,
    change: &str,
    documents: &Documents<'_>,
    lost: bool,
) -> Value {
    let (first, second) = if lost {
        (documents.then, documents.now)
    } else {
        (documents.now, documents.then)
    };
    let mut value = json!({ "role": role.as_str(), "change": change });
    match *subject {
        NodeRef::Held(node) => {
            let key = node.key();
            value["key"] = json!(key.to_string());
            if let Some(entry) = entry(&key, first, second) {
                value["kind"] = json!(entry.kind.as_str());
                value["label"] = json!(entry.label);
                value["name"] = json!(entry.name);
            } else if let Some(binding) =
                (documents.document.bindings.get(key)).or_else(|| documents.base.bindings.get(key))
            {
                value["label"] = json!(binding.label.to_string());
            }
        }
        NodeRef::Former(label) => value["label"] = json!(label.to_string()),
    }
    value
}

/// The `realizes` edges between boxes drawn, as the tree holds them, and as the base did where
/// the branch removed their Feature or Component; and the subject edges from each decision drawn
/// to the box of each node it names, where that box is drawn: an added or changed decision's as
/// the tree holds them, and a removed decision's as the base did.
fn edges(shown: &[Key], decided: &[Decided], documents: &Documents<'_>) -> Value {
    let Documents {
        base,
        document,
        now,
        ..
    } = *documents;
    let drawn: BTreeSet<Key> = shown.iter().copied().collect();
    let held = |removed: bool| -> BTreeSet<Key> {
        decided
            .iter()
            .filter(|decision| decision.removed == removed)
            .map(|decision| decision.key)
            .collect()
    };
    let (in_tree, in_base) = (held(false), held(true));
    // A base `realizes` edge between two nodes the tree holds is the tree's to draw, or not.
    let both_held = |edge: &&Edge| match **edge {
        Edge::Realizes { feature, component } => [feature.key(), component.key()]
            .iter()
            .all(|key| now.entries.contains_key(key)),
        _ => false,
    };
    let tree = (document.edges.iter())
        .filter_map(|edge| drawn_edge(edge, &drawn, &in_tree, &document.bindings));
    let gone = (base.edges.iter())
        .filter(|edge| !both_held(edge))
        .filter_map(|edge| drawn_edge(edge, &drawn, &in_base, &base.bindings));
    let found: BTreeSet<Drawn> = tree.chain(gone).collect();
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
    bindings: &Bindings,
) -> Option<Drawn> {
    match *edge {
        Edge::Realizes { feature, component } => {
            let (from, to) = (feature.key(), component.key());
            (drawn.contains(&from) && drawn.contains(&to))
                .then(|| (from.to_string(), to.to_string(), "realizes", None))
        }
        Edge::Subject {
            decision,
            subject,
            role,
        } if named.contains(&decision.key()) => {
            // A node named by its former label is the node the label's binding gives.
            let NodeRef::Held(node) = bindings.node(subject) else {
                return None;
            };
            let (from, to) = (decision.key(), node.key());
            drawn.contains(&to).then(|| {
                (
                    from.to_string(),
                    to.to_string(),
                    "subject",
                    Some(role.as_str()),
                )
            })
        }
        _ => None,
    }
}
