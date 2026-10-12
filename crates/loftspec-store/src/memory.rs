//! The memory store: the document held as the model, for tests and for a later server.

use loftspec::codec::{self, Files};
use loftspec::document::Document;
use loftspec::hash::{Roots, hash_document};

use crate::{DocumentStore, Saved, StoreError};

/// A document held in memory, beside the writer's output for it.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    held: Option<(Document, Files)>,
}

impl MemoryStore {
    /// A store that holds no document yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn document(&self) -> Result<&Document, StoreError> {
        self.held
            .as_ref()
            .map(|(document, _)| document)
            .ok_or(StoreError::NoDocument)
    }
}

impl DocumentStore for MemoryStore {
    fn load(&self) -> Result<Document, StoreError> {
        self.document().cloned()
    }

    /// Reports what a file store would write and remove for the same save, from the writer's
    /// output for the document held before and after.
    fn save(&mut self, document: &Document) -> Result<Saved, StoreError> {
        let next = codec::write(document);
        let saved = match &self.held {
            Some((_, current)) => Saved::between(current, &next),
            None => Saved::between(&Files::new(), &next),
        };
        self.held = Some((document.clone(), next));
        Ok(saved)
    }

    fn roots(&self) -> Result<Roots, StoreError> {
        hash_document(self.document()?)
            .map(|hashes| hashes.roots)
            .map_err(StoreError::Index)
    }
}
