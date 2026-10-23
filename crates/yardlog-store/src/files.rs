//! Yard Log's folder, `.yard/<branch>/` at the top of the worktree: reading its files, and the
//! changes that turn them into the files of Yard Log as it is to be written.

use std::path::{Path, PathBuf};

use loft_ports::journal::{Changes, RelPath};
use loft_ports::sync::read_if_present;
use yardlog::YardLog;
use yardlog::codec::ROOT;

use crate::YardStoreError;

/// The directory at the top of the worktree that holds every branch's Yard Log.
pub const YARD: &str = ".yard";

/// The line of `.gitattributes` that routes the files of Yard Log to `loft merge-driver`.
pub const ATTRIBUTE: &str = ".yard/**/*.yaml merge=loft";

/// One branch's folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Folder {
    tree: PathBuf,
    /// The folder's path from the top of the worktree.
    rel: String,
}

impl Folder {
    /// The folder of `branch`'s Yard Log, where each `/` of the branch's name separates
    /// directories.
    pub(crate) fn of(tree: &Path, branch: &str) -> Result<Self, YardStoreError> {
        let named = !branch.is_empty()
            && branch
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..")
            && !branch.contains('\\');
        if !named {
            return Err(YardStoreError::Branch(branch.to_owned()));
        }
        Ok(Self {
            tree: tree.to_owned(),
            rel: format!("{YARD}/{branch}"),
        })
    }

    pub(crate) fn rel(&self) -> &str {
        &self.rel
    }

    pub(crate) fn path(&self) -> PathBuf {
        self.tree.join(&self.rel)
    }

    /// A file's bytes, or none where it is not there.
    pub(crate) fn read(&self, name: &str) -> Result<Option<Vec<u8>>, YardStoreError> {
        Ok(read_if_present(&self.path().join(name))?)
    }

    /// The root's bytes, or none where Yard Log was never opened.
    pub(crate) fn root(&self) -> Result<Option<Vec<u8>>, YardStoreError> {
        self.read(ROOT)
    }

    /// The files of `log` whose bytes differ from the folder's, by their paths from the top of
    /// the worktree.
    pub(crate) fn changes(&self, log: &YardLog) -> Result<Changes, YardStoreError> {
        let mut changes = Changes::default();
        for (name, text) in log.files() {
            if self.read(&name)?.as_deref() != Some(text.as_bytes()) {
                changes.write.insert(
                    RelPath::new(format!("{}/{name}", self.rel)),
                    text.into_bytes(),
                );
            }
        }
        Ok(changes)
    }
}
