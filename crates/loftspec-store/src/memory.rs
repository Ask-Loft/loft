//! The memory store: the document held as the model, for tests and for a later server.

use loftspec::codec::{self, Files};
use loftspec::document::Document;

use crate::{DocumentStore, Saved, StoreError};

/// A document held in memory.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    document: Option<Document>,
}

impl MemoryStore {
    /// A store that holds no document yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl DocumentStore for MemoryStore {
    fn load(&self) -> Result<Document, StoreError> {
        self.document.clone().ok_or(StoreError::NoDocument)
    }

    /// Reports what a file store would write and remove for the same save, from the writer's
    /// output for the document held before and after.
    fn save(&mut self, document: &Document) -> Result<Saved, StoreError> {
        let current = self.document.as_ref().map_or_else(Files::new, codec::write);
        let saved = Saved::between(&current, &codec::write(document));
        self.document = Some(document.clone());
        Ok(saved)
    }
}
