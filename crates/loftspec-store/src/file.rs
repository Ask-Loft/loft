//! The file store: the document held as the `spec/` tree under a directory, read through the
//! codec and saved through the journal.

use std::fs;
use std::path::{Path, PathBuf};

use loftspec::codec::{self, Files, LABELS, SpecPath};
use loftspec::document::Document;
use loftspec::publish::{LOCK, Lock};

use crate::error::io_error;
use crate::fs::{SIDECAR, SPEC, hidden};
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

    /// Every file of the tree, by its path under `spec/`.
    ///
    /// # Errors
    ///
    /// A missing `spec/` directory, a file that cannot be read, or a name that is not UTF-8.
    pub fn read_files(&self) -> Result<Files, StoreError> {
        let spec = self.root.join(SPEC);
        if !spec.is_dir() {
            return Err(StoreError::NoTree(spec));
        }
        let mut files = Files::new();
        collect(&spec, "", &mut files)?;
        Ok(files)
    }

    /// Writes the given files of the tree in one save and removes none.
    ///
    /// # Errors
    ///
    /// A file that cannot be staged or put in place.
    pub fn write_files(&self, files: Files) -> Result<(), StoreError> {
        journal::save(&self.root, &Changes::of_tree(files, &[]))
    }

    /// Writes an imported tree into an empty or missing `spec/`, and the sidecar beside it, in
    /// one save.
    ///
    /// # Errors
    ///
    /// `spec/` holds a file or directory already (RQ-134), in which case nothing is written; or a
    /// file cannot be written.
    pub fn import(&self, files: Files, sidecar: &[u8]) -> Result<(), StoreError> {
        let spec = self.root.join(SPEC);
        if spec.exists() && holds_entries(&spec)? {
            return Err(StoreError::NotEmpty(spec));
        }
        fs::create_dir_all(&spec).map_err(io_error(&spec))?;
        self.write_with_beside(files, (SIDECAR, sidecar.to_vec()))
    }

    /// The lock beside `spec/`, or none.
    ///
    /// # Errors
    ///
    /// The lock is there but cannot be read, or is not a valid lock record.
    pub fn lock(&self) -> Result<Option<Lock>, StoreError> {
        let path = self.root.join(LOCK);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error(&path)(error)),
        };
        Lock::read(&bytes)
            .map(Some)
            .map_err(|error| StoreError::Lock(path, error))
    }

    /// Writes what a publish gives in one save: `spec/labels.yaml` when it changes, and the lock
    /// beside `spec/`; either both or neither (AC-463).
    ///
    /// # Errors
    ///
    /// A file cannot be staged or put in place.
    pub fn publish(&self, labels: Option<Vec<u8>>, lock: &Lock) -> Result<(), StoreError> {
        let mut files = Files::new();
        if let Some(labels) = labels {
            files.insert(SpecPath::new(LABELS), labels);
        }
        self.write_with_beside(files, (LOCK, lock.to_bytes()))
    }

    /// Writes files of the tree and one file beside it in one save, removing none.
    fn write_with_beside(
        &self,
        files: Files,
        (name, bytes): (&str, Vec<u8>),
    ) -> Result<(), StoreError> {
        let mut changes = Changes::of_tree(files, &[]);
        changes.write.insert(RootPath::beside(name), bytes);
        journal::save(&self.root, &changes)
    }
}

impl DocumentStore for FileStore {
    fn load(&self) -> Result<Document, StoreError> {
        codec::read(&self.read_files()?).map_err(StoreError::Read)
    }

    /// Writes each file whose bytes change and removes each file the document no longer has, in
    /// one save; a file whose bytes would not change is left alone. A save of this store's that
    /// failed part-way is finished first, so the tree it compares against is the one it left.
    fn save(&mut self, document: &Document) -> Result<Saved, StoreError> {
        journal::recover(&self.root)?;
        let current = match self.read_files() {
            Ok(files) => files,
            Err(StoreError::NoTree(_)) => Files::new(),
            Err(error) => return Err(error),
        };
        let mut next = codec::write(document);
        let saved = Saved::between(&current, &next);
        next.retain(|path, _| saved.written.binary_search(path).is_ok());
        journal::save(&self.root, &Changes::of_tree(next, &saved.removed))?;
        Ok(saved)
    }
}

/// Whether `dir` holds an entry that is not hidden.
fn holds_entries(dir: &Path) -> Result<bool, StoreError> {
    for entry in fs::read_dir(dir).map_err(io_error(dir))? {
        let entry = entry.map_err(io_error(dir))?;
        if !entry.file_name().to_str().is_some_and(hidden) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn collect(dir: &Path, prefix: &str, files: &mut Files) -> Result<(), StoreError> {
    for entry in fs::read_dir(dir).map_err(io_error(dir))? {
        let entry = entry.map_err(io_error(dir))?;
        let path = entry.path();
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| StoreError::NotUtf8(path.clone()))?;
        if hidden(&name) {
            continue;
        }
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if path.is_dir() {
            collect(&path, &relative, files)?;
        } else {
            let bytes = fs::read(&path).map_err(io_error(&path))?;
            files.insert(SpecPath::new(&relative), bytes);
        }
    }
    Ok(())
}
