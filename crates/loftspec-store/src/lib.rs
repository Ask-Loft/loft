//! Memory and file stores for Loft Specification Format documents: one interface that loads a
//! document, saves it and reports its roots, held in memory or as a `spec/` tree, so the same
//! scenarios run against both and the two agree.

pub mod file;
pub mod fs;
pub mod journal;
pub mod memory;

use loftspec::codec::{Files, Role, SpecPath};
use loftspec::document::Document;
use loftspec::hash::{Roots, hash_document};

pub use file::FileStore;
pub use fs::StoreError;
pub use memory::MemoryStore;

/// Where a document is held between commands.
pub trait DocumentStore {
    /// The document the store holds.
    ///
    /// # Errors
    ///
    /// The store holds no document, or its document cannot be read.
    fn load(&self) -> Result<Document, StoreError>;

    /// Replaces the document the store holds, and reports the files of the document's `spec/`
    /// tree that the save wrote and removed.
    ///
    /// # Errors
    ///
    /// The document cannot be saved; a file store then leaves every file as it was or finishes
    /// the save the next time it is opened.
    fn save(&mut self, document: &Document) -> Result<Saved, StoreError>;

    /// The roots of the document the store holds.
    ///
    /// # Errors
    ///
    /// As [`DocumentStore::load`], or the document does not index.
    fn roots(&self) -> Result<Roots, StoreError> {
        hash_document(&self.load()?)
            .map(|hashes| hashes.roots)
            .map_err(StoreError::Index)
    }
}

/// The files of the `spec/` tree a save wrote and removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Saved {
    pub written: Vec<SpecPath>,
    pub removed: Vec<SpecPath>,
}

impl Saved {
    /// What a save that turns the tree `current` into `next` writes and removes: each file whose
    /// bytes differ, and each file the writer owns that `next` no longer has. A file the document
    /// does not hold, such as `lint.yaml`, is never removed.
    #[must_use]
    pub fn between(current: &Files, next: &Files) -> Self {
        let written = next
            .iter()
            .filter(|(path, bytes)| current.get(*path) != Some(*bytes))
            .map(|(path, _)| path.clone())
            .collect();
        let removed = current
            .keys()
            .filter(|path| !next.contains_key(*path))
            .filter(|path| Role::of(path).is_some_and(|role| role != Role::Lint))
            .cloned()
            .collect();
        Self { written, removed }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.written.is_empty() && self.removed.is_empty()
    }
}
