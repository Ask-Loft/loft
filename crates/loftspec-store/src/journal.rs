//! A save through a journal beside `spec/`, the accepted decision "A save is journaled and
//! finished by the next command": the changed files are staged in `.loft/save/`, renaming the
//! journal's plan into place commits the save, and each staged file is then renamed into place.
//! A save interrupted before the commit leaves every file as it was; one interrupted after is
//! finished by [`recover`], which the file store runs when it is opened. The journal itself is
//! `loft-ports`'; this module names its directory and the paths of the tree.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use loft_ports::journal::{self as port, Journal, RelPath};
use loftspec::codec::{Files, SpecPath};
use serde::{Deserialize, Serialize};

use crate::StoreError;
use crate::fs::SPEC;

/// The journal's directory under the root.
pub const JOURNAL: &str = ".loft/save";

/// A file's path from the directory that holds `spec/`: a file of the tree, or one beside it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RootPath(String);

impl RootPath {
    /// A file of the tree.
    #[must_use]
    pub fn in_spec(path: &SpecPath) -> Self {
        Self(format!("{SPEC}/{}", path.as_str()))
    }

    /// A file beside `spec/`, such as the sidecar.
    #[must_use]
    pub fn beside(name: &str) -> Self {
        Self(name.to_owned())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// What one save writes and removes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    pub write: BTreeMap<RootPath, Vec<u8>>,
    pub remove: BTreeSet<RootPath>,
}

impl Changes {
    /// Writes each of the files of the tree and removes each of the paths under `spec/`.
    #[must_use]
    pub fn of_tree(write: Files, remove: &[SpecPath]) -> Self {
        Self {
            write: write
                .into_iter()
                .map(|(path, bytes)| (RootPath::in_spec(&path), bytes))
                .collect(),
            remove: remove.iter().map(RootPath::in_spec).collect(),
        }
    }

    fn port(&self) -> port::Changes {
        let path = |path: &RootPath| RelPath::new(path.as_str());
        port::Changes {
            write: self
                .write
                .iter()
                .map(|(target, bytes)| (path(target), bytes.clone()))
                .collect(),
            remove: self.remove.iter().map(path).collect(),
        }
    }
}

/// The journal of the tree under `root`, which removes a directory left empty only under
/// `spec/`.
fn journal(root: &Path) -> Journal {
    Journal::new(root, root.join(JOURNAL)).keeping(SPEC)
}

/// A save whose files are staged and whose plan is not yet committed.
#[derive(Debug)]
pub struct Prepared(port::Prepared);

/// A save whose plan is committed and whose files are not yet all in place.
#[derive(Debug)]
pub struct Committed(port::Committed);

/// Saves the changes: they are staged, committed and put in place. An empty set of changes
/// writes nothing.
///
/// # Errors
///
/// A journal is already there, or a file cannot be written, renamed or removed. Before the
/// commit every file is as it was; after it, [`recover`] finishes the save.
pub fn save(root: &Path, changes: &Changes) -> Result<(), StoreError> {
    Ok(journal(root).save(&changes.port())?)
}

/// Stages every file to write in the journal, each synced to disk.
///
/// # Errors
///
/// A journal is already there, or a staged file cannot be written.
pub fn prepare(root: &Path, changes: &Changes) -> Result<Prepared, StoreError> {
    Ok(Prepared(journal(root).prepare(&changes.port())?))
}

impl Prepared {
    /// Commits the save: the plan is written beside the staged files, synced, and renamed into
    /// place. From here on the save happens, whether or not this process finishes it.
    ///
    /// # Errors
    ///
    /// The plan cannot be written or renamed; the save is then not committed.
    pub fn commit(self) -> Result<Committed, StoreError> {
        Ok(Committed(self.0.commit()?))
    }
}

impl Committed {
    /// Puts every staged file in place, removes the files to remove, and deletes the journal.
    ///
    /// # Errors
    ///
    /// A file that cannot be renamed or removed; [`recover`] finishes the save later.
    pub fn finish(self) -> Result<(), StoreError> {
        Ok(self.0.finish()?)
    }
}

/// Finishes a committed save that was interrupted, or discards the staged files of one that was
/// not committed. With no journal it does nothing.
///
/// # Errors
///
/// The plan cannot be read or names an unknown format, or a file cannot be renamed or removed.
pub fn recover(root: &Path) -> Result<(), StoreError> {
    Ok(journal(root).recover()?)
}
