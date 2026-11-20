//! References into another document written by label or by name: the formatter replaces each
//! with the key of the one node of the copies under its handle, of a kind the field takes, that
//! carries that label or has that name.

use crate::codec::schema::{Kind, Target, field};
use crate::codec::{Problem, Rec, Val};
use crate::copy::Copy;
use crate::identity::Label;
use crate::node::Node;

/// A node of a copy, as a reference by label or by name finds it.
struct Held {
    handle: String,
    key: String,
    label: Option<Label>,
    name: String,
    /// Whether the node is the copied Interface itself.
    interface: bool,
}

/// Every node of every copy.
pub(super) struct Foreign(Vec<Held>);

impl Foreign {
    pub(super) fn of(copies: &[Copy]) -> Self {
        Self(copies.iter().flat_map(nodes_of).collect())
    }

    /// The reference into another document that `written`, a reference by label or by name in a
    /// field that takes `target`, stands for: only an Interface where the field takes one.
    pub(super) fn resolve(&self, written: &Rec, target: Option<Target>) -> Result<Val, Problem> {
        let handle = written.text(field::DOCUMENT).unwrap_or_default();
        let label = written.text(field::LABEL);
        let name = written.text(field::NAME);
        let label = label.map(|label| Label::parse(label).ok());
        let interface_only = target == Some(Target::Interface);
        let matches = |held: &&Held| match label {
            Some(label) => label.is_some() && held.label == label,
            None => name.is_some_and(|name| held.name == name),
        };
        let found: Vec<&str> = (self.0.iter())
            .filter(|held| held.handle == handle && (held.interface || !interface_only))
            .filter(matches)
            .map(|held| held.key.as_str())
            .collect();
        let selector = written.text(field::LABEL).or(name).unwrap_or_default();
        let wanted = format!("{handle}:{selector}");
        match found.as_slice() {
            [key] => {
                let mut reference = Rec::new(Kind::ForeignRef);
                reference.position = written.position;
                reference.set_text(field::DOCUMENT, handle);
                reference.set_text(field::KEY, *key);
                Ok(Val::Record(reference))
            }
            [] => Err(Problem::NameUnresolved {
                name: wanted,
                wanted: if interface_only {
                    "Interface of the copies under its handle"
                } else {
                    "node of the copies under its handle"
                },
            }),
            many => Err(Problem::NameAmbiguous {
                name: wanted,
                keys: many.iter().map(|key| (*key).to_owned()).collect(),
            }),
        }
    }
}

/// Every node of `copy` that has a key.
fn nodes_of(copy: &Copy) -> Vec<Held> {
    let mut held = Vec::new();
    Node::Interface(&copy.interface).walk(&mut |node| {
        if let Some(id) = node.id() {
            held.push(Held {
                handle: copy.handle.to_string(),
                key: id.key().to_string(),
                label: copy.labels.get(&id.key()).copied(),
                name: node.name().into_owned(),
                interface: matches!(node, Node::Interface(_)),
            });
        }
    });
    held
}
