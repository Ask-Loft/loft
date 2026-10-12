//! A save through a journal beside `spec/`, the accepted decision "A save is journaled and
//! finished by the next command": the changed files are staged in `.loft/save/`, renaming the
//! journal's plan into place commits the save, and each staged file is then renamed into place.
//! A save interrupted before the commit leaves every file as it was; one interrupted after is
//! finished by [`recover`], which the file store runs when it is opened.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use loftspec::codec::{Files, SpecPath};
use serde::{Deserialize, Serialize};

use crate::StoreError;
use crate::error::io_error;
use crate::fs::SPEC;

/// The journal's directory under the root.
pub const JOURNAL: &str = ".loft/save";

const PLAN: &str = "plan.json";
const PLAN_STAGED: &str = "plan.json.tmp";
const STAGED: &str = "files";
const FORMAT: &str = "loftspec-save/1";

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

    fn is_empty(&self) -> bool {
        self.write.is_empty() && self.remove.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Plan {
    format: String,
    write: Vec<Staged>,
    remove: Vec<RootPath>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Staged {
    path: RootPath,
    staged: String,
}

/// A save whose files are staged and whose plan is not yet committed.
#[derive(Debug)]
pub struct Prepared {
    root: PathBuf,
    plan: Plan,
}

/// A save whose plan is committed and whose files are not yet all in place.
#[derive(Debug)]
pub struct Committed(Prepared);

/// Saves the changes: they are staged, committed and put in place. An empty set of changes
/// writes nothing.
///
/// # Errors
///
/// A journal is already there, or a file cannot be written, renamed or removed. Before the
/// commit every file is as it was; after it, [`recover`] finishes the save.
pub fn save(root: &Path, changes: &Changes) -> Result<(), StoreError> {
    if changes.is_empty() {
        return Ok(());
    }
    prepare(root, changes)?.commit()?.finish()
}

/// Stages every file to write in the journal, each synced to disk.
///
/// # Errors
///
/// A journal is already there, or a staged file cannot be written.
pub fn prepare(root: &Path, changes: &Changes) -> Result<Prepared, StoreError> {
    let journal = root.join(JOURNAL);
    if journal.exists() {
        return Err(StoreError::Pending(journal));
    }
    let staged_dir = journal.join(STAGED);
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
        root: root.to_owned(),
        plan: Plan {
            format: FORMAT.to_owned(),
            write,
            remove: changes.remove.iter().cloned().collect(),
        },
    })
}

impl Prepared {
    /// Commits the save: the plan is written beside the staged files, synced, and renamed into
    /// place. From here on the save happens, whether or not this process finishes it.
    ///
    /// # Errors
    ///
    /// The plan cannot be written or renamed; the save is then not committed.
    pub fn commit(self) -> Result<Committed, StoreError> {
        let journal = self.root.join(JOURNAL);
        let staged = journal.join(PLAN_STAGED);
        let bytes = serde_json::to_vec_pretty(&self.plan).expect("a plan serializes");
        write_synced(&staged, &bytes)?;
        let plan = journal.join(PLAN);
        fs::rename(&staged, &plan).map_err(io_error(&plan))?;
        sync_dir(&journal)?;
        Ok(Committed(self))
    }
}

impl Committed {
    /// Puts every staged file in place, removes the files to remove, and deletes the journal.
    ///
    /// # Errors
    ///
    /// A file that cannot be renamed or removed; [`recover`] finishes the save later.
    pub fn finish(self) -> Result<(), StoreError> {
        finish(&self.0.root, &self.0.plan)
    }
}

/// Finishes a committed save that was interrupted, or discards the staged files of one that was
/// not committed. With no journal it does nothing.
///
/// # Errors
///
/// The plan cannot be read or names an unknown format, or a file cannot be renamed or removed.
pub fn recover(root: &Path) -> Result<(), StoreError> {
    let journal = root.join(JOURNAL);
    if !journal.exists() {
        return Ok(());
    }
    let plan_path = journal.join(PLAN);
    match fs::read(&plan_path) {
        Ok(bytes) => {
            let plan: Plan = serde_json::from_slice(&bytes)
                .map_err(|error| StoreError::Journal(plan_path.clone(), error.to_string()))?;
            if plan.format != FORMAT {
                return Err(StoreError::Journal(
                    plan_path,
                    format!("unknown format `{}`", plan.format),
                ));
            }
            finish(root, &plan)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => remove_journal(root),
        Err(error) => Err(io_error(&plan_path)(error)),
    }
}

/// Every step skips what an earlier, interrupted run already did, so finishing twice is safe.
fn finish(root: &Path, plan: &Plan) -> Result<(), StoreError> {
    let staged_dir = root.join(JOURNAL).join(STAGED);
    let mut touched = BTreeSet::new();
    for entry in &plan.write {
        let staged = staged_dir.join(&entry.staged);
        let target = root.join(entry.path.as_str());
        if staged.exists() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(io_error(parent))?;
            }
            fs::rename(&staged, &target).map_err(io_error(&target))?;
        }
        touched.extend(target.parent().map(Path::to_owned));
    }
    for path in &plan.remove {
        let target = root.join(path.as_str());
        match fs::remove_file(&target) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(&target)(error)),
        }
        remove_empty_parents(root, &target);
        touched.extend(target.parent().map(Path::to_owned));
    }
    // A directory a later removal emptied is gone, and has nothing left to sync.
    for dir in touched.iter().filter(|dir| dir.exists()) {
        sync_dir(dir)?;
    }
    remove_journal(root)
}

/// Removes each directory above `target` that is left empty, up to but not including `spec/`.
fn remove_empty_parents(root: &Path, target: &Path) {
    let spec = root.join(SPEC);
    let mut dir = target.parent();
    while let Some(current) = dir {
        if current == spec || !current.starts_with(&spec) || fs::remove_dir(current).is_err() {
            break;
        }
        dir = current.parent();
    }
}

/// Deletes the journal, and `.loft/` too when nothing else is in it.
fn remove_journal(root: &Path) -> Result<(), StoreError> {
    let journal = root.join(JOURNAL);
    fs::remove_dir_all(&journal).map_err(io_error(&journal))?;
    if let Some(parent) = journal.parent() {
        let _ = fs::remove_dir(parent);
    }
    Ok(())
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let mut file = fs::File::create(path).map_err(io_error(path))?;
    file.write_all(bytes).map_err(io_error(path))?;
    file.sync_all().map_err(io_error(path))
}

/// Syncs a directory, so the renames into it survive a power loss. Only Unix can open a
/// directory to sync it.
fn sync_dir(dir: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    fs::File::open(dir)
        .and_then(|handle| handle.sync_all())
        .map_err(io_error(dir))?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}
