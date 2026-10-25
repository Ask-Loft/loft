//! A tree opened once: its store and the files of its `spec/`, read for a command, or for the
//! `loft` views of Yard Log, which report on one tree as several commands would.

use std::path::Path;

use loftspec::codec::Files;
use loftspec_store::FileStore;

use crate::stop::Stop;

/// The store of the tree under a root, and the files of its `spec/`.
pub struct Opened {
    pub store: FileStore,
    pub files: Files,
}

impl Opened {
    /// Opens the tree under `root`, finishing or discarding an interrupted save, and reads its
    /// files.
    ///
    /// # Errors
    ///
    /// The store cannot be opened, or `spec/` cannot be read.
    pub fn open(root: &Path) -> Result<Self, Stop> {
        let store = FileStore::open(root)?;
        let files = store.read_files()?;
        Ok(Self { store, files })
    }
}
