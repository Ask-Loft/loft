//! The file store: the document held as the `spec/` tree under a directory, read through the
//! codec and saved through the journal.

use std::path::{Path, PathBuf};

use loftspec::codec::{self, Files};
use loftspec::document::Document;

use crate::fs::{read_tree, write_changed};
use crate::journal::{self, Changes, RootPath};
use crate::{DocumentStore, Saved, StoreError};

/// The `spec/` tree under a directory.
#[derive(Debug, Clone)]
pub struct FileStore {
    root: PathBuf,
}

impl FileStore {
    /// The store of the tree under `root`, after finishing a committed save there that was
    /// interrupted, or discarding an uncommitted one.
    ///
    /// # Errors
    ///
    /// An interrupted save that cannot be recovered.
    pub fn open(root: &Path) -> Result<Self, StoreError> {
        journal::recover(root)?;
        Ok(Self {
            root: root.to_owned(),
        })
    }

    /// The directory that holds `spec/`.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every file of the tree, by its path under `spec/`.
    ///
    /// # Errors
    ///
    /// A missing `spec/` directory, a file that cannot be read, or a name that is not UTF-8.
    pub fn read_files(&self) -> Result<Files, StoreError> {
        read_tree(&self.root)
    }

    /// Writes the given files of the tree in one save and removes none.
    ///
    /// # Errors
    ///
    /// A file that cannot be staged or put in place.
    pub fn write_files(&self, files: &Files) -> Result<(), StoreError> {
        write_changed(&self.root, files)
    }
}

impl DocumentStore for FileStore {
    fn load(&self) -> Result<Document, StoreError> {
        codec::read(&self.read_files()?).map_err(StoreError::Read)
    }

    /// Writes each file whose bytes change and removes each file the document no longer has, in
    /// one save; a file whose bytes would not change is left alone.
    fn save(&mut self, document: &Document) -> Result<Saved, StoreError> {
        let current = match self.read_files() {
            Ok(files) => files,
            Err(StoreError::NoTree(_)) => Files::new(),
            Err(error) => return Err(error),
        };
        let mut next = codec::write(document);
        let saved = Saved::between(&current, &next);
        let changes = Changes {
            write: saved
                .written
                .iter()
                .map(|path| {
                    let bytes = next.remove(path).expect("a written path is in the output");
                    (RootPath::in_spec(path), bytes)
                })
                .collect(),
            remove: saved.removed.iter().map(RootPath::in_spec).collect(),
        };
        journal::save(&self.root, &changes)?;
        Ok(saved)
    }
}
