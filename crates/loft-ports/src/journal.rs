//! A save through a journal, the accepted decision "A save is journaled and finished by the next
//! command": the changed files are staged in the journal's directory, renaming the journal's
//! plan into place commits the save, and each staged file is then moved into place. A save
//! interrupted before the commit leaves every file as it was; one interrupted after is finished
//! by [`Journal::recover`], which the next command runs before it reads.
//!
//! The paths it saves are plain paths from a root directory, and the journal's directory is the
//! caller's to name: beside `spec/` for a document, in git's own directory for Yard Log.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::plan::{FORMAT, PLAN, PLAN_STAGED, Plan, STAGED, Staged};
use crate::sync::{sync_dir, write_synced};
use crate::{PortError, check, io_error};

/// A file's path from the root, its parts separated by `/`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelPath(String);

impl RelPath {
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What one save writes and removes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    pub write: BTreeMap<RelPath, Vec<u8>>,
    pub remove: BTreeSet<RelPath>,
}

impl Changes {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.write.is_empty() && self.remove.is_empty()
    }
}

/// The journal of the files under one root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Journal {
    root: PathBuf,
    dir: PathBuf,
    floor: Option<String>,
    beside: BTreeSet<String>,
}

impl Journal {
    /// The journal that saves files under `root`, staged in `dir`, which only one save at a time
    /// uses and which need not lie under `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, dir: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            dir: dir.into(),
            floor: None,
            beside: BTreeSet::new(),
        }
    }

    /// The same journal, which saves only files under `floor`, a directory under the root, and
    /// removes a directory a removal leaves empty only below it; without a floor, any file and
    /// any directory below the root.
    #[must_use]
    pub fn keeping(mut self, floor: &str) -> Self {
        self.floor = Some(floor.to_owned());
        self
    }

    /// The same journal, which also saves the files at `paths`, outside its floor.
    #[must_use]
    pub fn beside(mut self, paths: &[&str]) -> Self {
        self.beside
            .extend(paths.iter().map(|&path| path.to_owned()));
        self
    }

    /// Whether the journal saves the file at `path`: any with no floor, or else one under the
    /// floor or named beside it.
    pub(crate) fn saves(&self, path: &str) -> bool {
        self.floor.is_none() || self.under_floor(path) || self.beside.contains(path)
    }

    /// Whether `path` lies under the journal's floor, where it has one.
    pub(crate) fn under_floor(&self, path: &str) -> bool {
        self.floor.as_ref().is_some_and(|floor| {
            let under = path.strip_prefix(floor.as_str());
            under.is_some_and(|rest| rest.starts_with('/'))
        })
    }

    /// The floor's path from the root, where the journal has one.
    pub(crate) fn floor_name(&self) -> Option<&str> {
        self.floor.as_deref()
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn floor(&self) -> PathBuf {
        self.floor
            .as_ref()
            .map_or_else(|| self.root.clone(), |floor| self.root.join(floor))
    }

    /// Saves the changes: they are staged, committed and put in place. An empty set of changes
    /// writes nothing.
    ///
    /// # Errors
    ///
    /// A journal is already there, a path is not one the journal saves, or a file cannot be
    /// written, renamed or removed. Before the commit every file is as it was; after it,
    /// [`Journal::recover`] finishes the save.
    pub fn save(&self, changes: &Changes) -> Result<(), PortError> {
        if changes.is_empty() {
            return Ok(());
        }
        self.prepare(changes)?.commit()?.finish()
    }

    /// Stages every file to write in the journal, each synced to disk.
    ///
    /// # Errors
    ///
    /// A journal is already there, a path is not one the journal saves, or a staged file
    /// cannot be written.
    pub fn prepare(&self, changes: &Changes) -> Result<Prepared, PortError> {
        if self.dir.exists() {
            return Err(PortError::Pending(self.dir.clone()));
        }
        let named = changes.write.keys().chain(&changes.remove);
        check::paths(self, named.map(RelPath::as_str)).map_err(|refused| {
            PortError::Journal(
                self.dir.clone(),
                format!(
                    "a save cannot write or remove `{}`, which {}",
                    refused.path, refused.why
                ),
            )
        })?;
        let staged_dir = self.dir.join(STAGED);
        fs::create_dir_all(&staged_dir).map_err(io_error(&staged_dir))?;
        let mut write = Vec::with_capacity(changes.write.len());
        for (index, (path, bytes)) in changes.write.iter().enumerate() {
            let staged = index.to_string();
            write_synced(&staged_dir.join(&staged), bytes)?;
            write.push(Staged {
                path: path.clone(),
                staged,
            });
        }
        sync_dir(&staged_dir)?;
        Ok(Prepared {
            journal: self.clone(),
            plan: Plan {
                format: FORMAT.to_owned(),
                write,
                remove: changes.remove.iter().cloned().collect(),
            },
        })
    }

    /// Finishes a committed save that was interrupted, or discards the staged files of one that
    /// was not committed. With no journal it does nothing.
    ///
    /// # Errors
    ///
    /// The journal's directory lies behind a link, or the plan cannot be read, names an unknown
    /// format, or names a path or a staged file that is not the journal's to save, in each of
    /// which cases no file changes; or a file cannot be renamed or removed.
    pub fn recover(&self) -> Result<(), PortError> {
        if !self.dir.exists() {
            return Ok(());
        }
        // Before the plan is read and before an uncommitted journal is deleted: either would
        // act on whatever a link leads to.
        check::place(self, STAGED)
            .map_err(|why| PortError::Journal(self.dir.clone(), Plan::not_a_save(None, why)))?;
        match Plan::read(self)? {
            Some(plan) => plan.finish(self),
            None => self.discard(),
        }
    }

    /// Deletes the journal's directory, and the directory that holds it too when nothing else
    /// is in it.
    pub(crate) fn discard(&self) -> Result<(), PortError> {
        fs::remove_dir_all(&self.dir).map_err(io_error(&self.dir))?;
        if let Some(parent) = self.dir.parent() {
            let _ = fs::remove_dir(parent);
        }
        Ok(())
    }
}

/// A save whose files are staged and whose plan is not yet committed.
#[derive(Debug)]
pub struct Prepared {
    journal: Journal,
    plan: Plan,
}

/// A save whose plan is committed and whose files are not yet all in place.
#[derive(Debug)]
pub struct Committed(Prepared);

impl Prepared {
    /// Commits the save: the plan is written beside the staged files, synced, and renamed into
    /// place. From here on the save happens, whether or not this process finishes it.
    ///
    /// # Errors
    ///
    /// The plan cannot be written or renamed; the save is then not committed.
    pub fn commit(self) -> Result<Committed, PortError> {
        let dir = self.journal.dir();
        let staged = dir.join(PLAN_STAGED);
        let bytes = serde_json::to_vec_pretty(&self.plan).expect("a plan serializes");
        write_synced(&staged, &bytes)?;
        let plan = dir.join(PLAN);
        fs::rename(&staged, &plan).map_err(io_error(&plan))?;
        sync_dir(dir)?;
        Ok(Committed(self))
    }
}

impl Committed {
    /// Puts every staged file in place, removes the files to remove, and deletes the journal.
    ///
    /// # Errors
    ///
    /// A file that cannot be moved or removed; [`Journal::recover`] finishes the save later.
    pub fn finish(self) -> Result<(), PortError> {
        self.0.plan.finish(&self.0.journal)
    }
}
