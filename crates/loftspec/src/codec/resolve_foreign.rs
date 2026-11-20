//! A reference into another document, resolved against the copies under its handle: only a
//! node that a copy under the handle holds can be named.

use super::error::ReadError;
use super::record::Rec;
use super::resolve::Resolver;
use super::schema::field;
use crate::identity::Key;
use crate::ids::NodeId;

impl Resolver<'_> {
    /// The node of a copy that a reference into another document names.
    pub(super) fn foreign_node(&self, reference: &Rec) -> Result<NodeId, ReadError> {
        let handle = reference.text(field::DOCUMENT).unwrap_or_default();
        let text = reference.text(field::KEY).unwrap_or_default();
        let key = Key::parse(text).expect("a checked key");
        let listed = (self.document.documents.iter()).any(|other| other.handle.as_str() == handle);
        if !listed {
            let problem = format!(
                "the reference to {handle}:{key} names a handle that the manifest's `documents` \
                 does not list"
            );
            return Err(self.fail(Some(reference.position), problem));
        }
        self.foreign
            .get(&(handle.to_owned(), key))
            .copied()
            .ok_or_else(|| {
                let problem = format!(
                    "the reference to {handle}:{key} names a key that no copy under the handle \
                     {handle} holds"
                );
                self.fail(Some(reference.position), problem)
            })
    }
}
