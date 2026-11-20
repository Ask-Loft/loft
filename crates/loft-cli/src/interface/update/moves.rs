//! What an update does to one copy: leaves it current, finds its Interface withdrawn, or moves
//! its pin, with the nodes the move added, changed and removed.

use loftspec::codec::{self, CopyError};
use loftspec::copy::Copy;
use loftspec::diff::{NodeChanges, interface_changes};
use loftspec::document::Document;
use loftspec::identity::Key;
use loftspec::ids::NodeId;
use loftspec::node::Node;
use serde_json::{Value, json};

use super::super::reach::{Remotes, not_held};
use super::super::{entry_of, refused};
use crate::stop::Stop;

/// What an update did to one copy.
pub(super) enum Move<'a> {
    /// The copy is pinned to the publish sequence it would move to.
    Current { copy: &'a Copy },
    /// The state it would move to no longer holds its Interface; it stays at its pin.
    Withdrawn { copy: &'a Copy, head: u64 },
    /// The copy moved, with the nodes the move added, changed and removed.
    Moved {
        from: &'a Copy,
        to: Box<Copy>,
        changes: NodeChanges,
    },
}

impl Move<'_> {
    fn copy(&self) -> &Copy {
        match self {
            Self::Current { copy } | Self::Withdrawn { copy, .. } => copy,
            Self::Moved { from, .. } => from,
        }
    }

    /// The copied Interface as `<handle>:<label>`.
    fn name(&self) -> String {
        let copy = self.copy();
        foreign_name(copy, copy.keys()[0])
    }

    /// The lines that report the move.
    pub fn lines(&self) -> Vec<String> {
        let name = self.name();
        match self {
            Self::Current { copy } => vec![format!(
                "{name} is current at publish sequence {}",
                copy.publish_sequence
            )],
            Self::Withdrawn { copy, head } => vec![format!(
                "{name} is withdrawn: publish sequence {head} no longer holds it, and its copy \
                 stays at publish sequence {}",
                copy.publish_sequence
            )],
            Self::Moved { from, to, changes } => {
                let mut lines = vec![format!(
                    "updated {name} from publish sequence {} to {}",
                    from.publish_sequence, to.publish_sequence
                )];
                lines.extend(
                    parts(from, to, changes)
                        .into_iter()
                        .flat_map(|(verb, copy, ids)| {
                            (ids.iter())
                                .map(move |id| format!("  {verb} {}", described(copy, *id).0))
                        }),
                );
                lines
            }
        }
    }

    pub fn json(&self) -> Value {
        let (state, head, nodes) = match self {
            Self::Current { copy } => ("current", copy.publish_sequence, json!({})),
            Self::Withdrawn { head, .. } => ("withdrawn", *head, json!({})),
            Self::Moved { from, to, changes } => {
                let mut nodes = serde_json::Map::new();
                for (verb, copy, ids) in parts(from, to, changes) {
                    let listed: Vec<Value> =
                        (ids.iter()).map(|id| described(copy, *id).1).collect();
                    nodes.insert(verb.to_owned(), Value::Array(listed));
                }
                ("updated", to.publish_sequence, Value::Object(nodes))
            }
        };
        let empty = Vec::new;
        json!({
            "interface": self.name(),
            "state": state,
            "publishSequence": match self {
                Self::Moved { to, .. } => to.publish_sequence,
                _ => self.copy().publish_sequence,
            },
            "head": head,
            "added": nodes.get("added").cloned().unwrap_or_else(|| Value::Array(empty())),
            "changed": nodes.get("changed").cloned().unwrap_or_else(|| Value::Array(empty())),
            "removed": nodes.get("removed").cloned().unwrap_or_else(|| Value::Array(empty())),
        })
    }
}

/// The added and changed nodes, read from the copy the move made, and the removed, read from
/// the copy it replaced.
fn parts<'b>(
    from: &'b Copy,
    to: &'b Copy,
    changes: &'b NodeChanges,
) -> [(&'static str, &'b Copy, &'b [NodeId]); 3] {
    [
        ("added", to, &changes.added),
        ("changed", to, &changes.changed),
        ("removed", from, &changes.removed),
    ]
}

/// The node `id` of `copy`, as a line and as JSON: its foreign label and its name.
fn described(copy: &Copy, id: NodeId) -> (String, Value) {
    let mut name = String::new();
    Node::Interface(&copy.interface).walk(&mut |node| {
        if node.id() == Some(id) {
            name = node.name().into_owned();
        }
    });
    let label = foreign_name(copy, id.key());
    (
        format!("{label} · {name}"),
        json!({ "label": label, "name": name }),
    )
}

/// The node `key` of `copy` as `<handle>:<label>`, or `<handle>:<key>` where it has no label.
pub(in super::super) fn foreign_name(copy: &Copy, key: Key) -> String {
    match copy.labels.get(&key) {
        Some(label) => format!("{}:{label}", copy.handle),
        None => format!("{}:{key}", copy.handle),
    }
}

/// What an update does to `copy`, its remote reached through `remotes`.
impl Move<'_> {
    /// Moves `copy` of `document` to its remote's head, or to the publish sequence `at`.
    pub fn of<'a>(
        remotes: &mut Remotes,
        document: &Document,
        copy: &'a Copy,
        at: Option<u64>,
    ) -> Result<Move<'a>, Stop> {
        let handle = &copy.handle;
        let entry = entry_of(document, copy);
        let sequence = match at {
            Some(at) => at,
            None => remotes.head(entry)?,
        };
        if sequence == copy.publish_sequence {
            return Ok(Move::Current { copy });
        }
        let (files, lock) = remotes
            .state(entry, sequence)?
            .ok_or_else(|| not_held(&entry.remote, sequence))?;
        let interface = copy.keys()[0];
        match codec::copy_of(handle, files, interface, sequence, lock.state_root) {
            Ok(to) => Ok(Move::Moved {
                changes: interface_changes(&copy.interface, &to.interface),
                from: copy,
                to: Box::new(to),
            }),
            Err(CopyError::NotHeld(_)) => Ok(Move::Withdrawn {
                copy,
                head: sequence,
            }),
            Err(CopyError::Unreadable(errors)) => Err(refused(format!(
                "{handle}: the published state of publish sequence {sequence} does not read: {}",
                errors.first().map(ToString::to_string).unwrap_or_default()
            ))),
        }
    }
}
