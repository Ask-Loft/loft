//! References into another document written by label or by name: the formatter replaces each
//! with the key of the one node of the copies under its handle that carries that label or has
//! that name.

use crate::codec::schema::{Kind, field};
use crate::codec::{Problem, Rec, Val};
use crate::copy::Copy;
use crate::identity::Label;
use crate::node::Node;

/// Every node of every copy, as its handle, its key, its label and its name.
pub(super) struct Foreign(Vec<(String, String, Option<Label>, String)>);

impl Foreign {
    pub(super) fn of(copies: &[Copy]) -> Self {
        Self(copies.iter().flat_map(nodes_of).collect())
    }

    /// The reference into another document that `written`, a reference by label or by name,
    /// stands for.
    pub(super) fn resolve(&self, written: &Rec) -> Result<Val, Problem> {
        let handle = written.text(field::DOCUMENT).unwrap_or_default();
        let label = written.text(field::LABEL);
        let name = written.text(field::NAME);
        let matches = |(_, _, held, named): &&(String, String, Option<Label>, String)| match label {
            Some(label) => held.is_some_and(|held| held.to_string() == label),
            None => name.is_some_and(|name| named == name),
        };
        let found: Vec<&str> = (self.0.iter())
            .filter(|(under, ..)| under == handle)
            .filter(matches)
            .map(|(_, key, ..)| key.as_str())
            .collect();
        let wanted = format!("{handle}:{}", label.or(name).unwrap_or_default());
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
                wanted: "node of the copies under its handle",
            }),
            many => Err(Problem::NameAmbiguous {
                name: wanted,
                keys: many.iter().map(|key| (*key).to_owned()).collect(),
            }),
        }
    }
}

/// Every node of `copy` that has a key, as its handle, its key, its label and its name.
fn nodes_of(copy: &Copy) -> Vec<(String, String, Option<Label>, String)> {
    let mut held = Vec::new();
    Node::Interface(&copy.interface).walk(&mut |node| held.push(node));
    let keyed = held.into_iter().filter_map(|node| Some((node.id()?, node)));
    keyed
        .map(|(id, node)| {
            let label = copy.labels.get(&id.key()).copied();
            let name = node.name().into_owned();
            (copy.handle.to_string(), id.key().to_string(), label, name)
        })
        .collect()
}
