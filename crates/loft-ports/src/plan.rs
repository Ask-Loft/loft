//! The journal's plan: what a committed save writes and removes, as `plan.json` records it, and
//! finishing it.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::journal::{Journal, RelPath};
use crate::sync::{move_into_place, sync_dir};
use crate::{PortError, io_error};

pub(crate) const PLAN: &str = "plan.json";
pub(crate) const PLAN_STAGED: &str = "plan.json.tmp";
pub(crate) const STAGED: &str = "files";
/// The plan's format, unchanged since the journal served `spec/` alone, so that a save an
/// earlier release left committed is finished by this one.
pub(crate) const FORMAT: &str = "loftspec-save/1";

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Plan {
    pub(crate) format: String,
    pub(crate) write: Vec<Staged>,
    pub(crate) remove: Vec<RelPath>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Staged {
    pub(crate) path: RelPath,
    pub(crate) staged: String,
}

impl Plan {
    /// Reads the plan of a committed save, where there is one.
    pub(crate) fn read(journal: &Journal) -> Result<Option<Self>, PortError> {
        let path = journal.dir().join(PLAN);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error(&path)(error)),
        };
        let plan: Self = serde_json::from_slice(&bytes)
            .map_err(|error| PortError::Journal(path.clone(), error.to_string()))?;
        if plan.format != FORMAT {
            return Err(PortError::Journal(
                path,
                format!("unknown format `{}`", plan.format),
            ));
        }
        Ok(Some(plan))
    }

    /// Puts every staged file in place, removes the files to remove, and deletes the journal.
    /// Every step skips what an earlier, interrupted run already did, so finishing twice is
    /// safe.
    pub(crate) fn finish(&self, journal: &Journal) -> Result<(), PortError> {
        let staged_dir = journal.dir().join(STAGED);
        let mut touched = BTreeSet::new();
        for entry in &self.write {
            let staged = staged_dir.join(&entry.staged);
            let target = journal.root().join(entry.path.as_str());
            if staged.exists() {
                put(&staged, &target)?;
            }
            touched.extend(target.parent().map(Path::to_owned));
        }
        for path in &self.remove {
            let target = journal.root().join(path.as_str());
            match fs::remove_file(&target) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(io_error(&target)(error)),
            }
            remove_empty_parents(&journal.floor(), &target);
            touched.extend(target.parent().map(Path::to_owned));
        }
        // A directory a later removal emptied is gone, and has nothing left to sync.
        for dir in touched.iter().filter(|dir| dir.exists()) {
            sync_dir(dir)?;
        }
        journal.discard()
    }
}

/// Moves a staged file to its target, creating the directories that hold it.
fn put(staged: &Path, target: &Path) -> Result<(), PortError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(io_error(parent))?;
    }
    move_into_place(staged, target)
}

/// Removes each directory above `target` that is left empty, up to but not including `floor`,
/// and none outside it.
fn remove_empty_parents(floor: &Path, target: &Path) {
    let mut dir = target.parent();
    while let Some(current) = dir {
        if current == floor || !current.starts_with(floor) || fs::remove_dir(current).is_err() {
            break;
        }
        dir = current.parent();
    }
}
