//! What an update does to one copy: leaves it current, finds its Interface withdrawn, or moves
//! its pin, with the nodes the move added, changed and removed.

use std::collections::BTreeMap;

use loftspec::codec::{self, CopyError};
use loftspec::copy::Copy;
use loftspec::diff::{NodeChanges, interface_changes};
use loftspec::document::Document;
use loftspec::ids::NodeId;
use loftspec::node::Node;
use serde_json::{Value, json};

use super::super::entry_of;
use super::super::reach::{Remotes, not_held, unreadable};
use crate::stop::Stop;

/// What an update did to one copy.
pub(super) enum Move<'a> {
    /// The copy is pinned to the publish sequence it would move to, and is the copy that state
    /// gives.
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
        copy.name_of(copy.key())
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
                let mut lines = vec![if from.publish_sequence == to.publish_sequence {
                    format!(
                        "took {name} again at publish sequence {}: its copy was not the one its \
                         remote published",
                        to.publish_sequence
                    )
                } else {
                    format!(
                        "updated {name} from publish sequence {} to {}",
                        from.publish_sequence, to.publish_sequence
                    )
                }];
                for (verb, copy, ids) in parts(from, to, changes) {
                    let nodes = described(copy, ids).into_iter();
                    lines.extend(nodes.map(|(label, name)| format!("  {verb} {label} · {name}")));
                }
                lines
            }
        }
    }

    pub fn json(&self) -> Value {
        let (state, sequence, head) = match self {
            Self::Current { copy } => ("current", copy.publish_sequence, copy.publish_sequence),
            Self::Withdrawn { copy, head } => ("withdrawn", copy.publish_sequence, *head),
            Self::Moved { to, .. } => ("updated", to.publish_sequence, to.publish_sequence),
        };
        let mut document = json!({
            "interface": self.name(),
            "state": state,
            "publishSequence": sequence,
            "head": head,
            "added": [],
            "changed": [],
            "removed": [],
        });
        if let Self::Moved { from, to, changes } = self {
            for (verb, copy, ids) in parts(from, to, changes) {
                let nodes = described(copy, ids).into_iter();
                document[verb] = nodes
                    .map(|(label, name)| json!({ "label": label, "name": name }))
                    .collect();
            }
        }
        document
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

/// The nodes `ids` of `copy`, each as its foreign label and its name.
fn described(copy: &Copy, ids: &[NodeId]) -> Vec<(String, String)> {
    let mut names = BTreeMap::new();
    Node::Interface(&copy.interface).walk(&mut |node| {
        if let Some(id) = node.id() {
            names.insert(id, node.name().into_owned());
        }
    });
    (ids.iter())
        .map(|id| (copy.name_of(id.key()), names.remove(id).unwrap_or_default()))
        .collect()
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
        let (files, lock) = remotes
            .state(entry, sequence)?
            .ok_or_else(|| not_held(&entry.remote, sequence))?;
        let interface = copy.key();
        match codec::copy_of(handle, files, interface, sequence, lock.state_root) {
            // A copy at the head stays as it is, unless it is not the copy its state gives.
            Ok(to) if sequence == copy.publish_sequence && copy.difference(&to).is_none() => {
                Ok(Move::Current { copy })
            }
            Ok(to) => Ok(Move::Moved {
                changes: interface_changes(&copy.interface, &to.interface),
                from: copy,
                to: Box::new(to),
            }),
            Err(CopyError::NotHeld(_)) => Ok(Move::Withdrawn {
                copy,
                head: sequence,
            }),
            Err(CopyError::Unreadable(errors)) => Err(unreadable(handle, sequence, &errors)),
        }
    }
}
