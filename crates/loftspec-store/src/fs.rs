//! The `spec/` tree on disk: read whole, and written back through the journal (AC-537), so a
//! crash leaves every file either as it was or as written.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::{fs, io, process};

use loftspec::codec::{Files, SpecPath};

use crate::journal::{self, Changes, RootPath};

/// The directory under the root that holds the tree.
pub const SPEC: &str = "spec";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{0}: there is no `spec/` directory")]
    NoTree(PathBuf),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{0}: a file name that is not UTF-8")]
    NotUtf8(PathBuf),
    #[error("{0}: `spec/` is not empty")]
    NotEmpty(PathBuf),
    #[error("{0}: a save is already in progress")]
    Pending(PathBuf),
    #[error("{0}: {1}")]
    Journal(PathBuf, String),
}

pub(crate) fn io_error(path: &Path) -> impl FnOnce(io::Error) -> StoreError + '_ {
    move |source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Every file under `<root>/spec/`, by its path under `spec/`.
///
/// # Errors
///
/// A missing `spec/` directory, a file that cannot be read, or a name that is not UTF-8.
pub fn read_tree(root: &Path) -> Result<Files, StoreError> {
    let spec = root.join(SPEC);
    if !spec.is_dir() {
        return Err(StoreError::NoTree(spec));
    }
    let mut files = BTreeMap::new();
    collect(&spec, "", &mut files)?;
    Ok(files)
}

/// Whether an entry under `spec/` is left out of the tree: its name begins with `.`, as an
/// operating system's or an editor's own files do (`.DS_Store`, swap files).
fn hidden(name: &str) -> bool {
    name.starts_with('.')
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

/// Writes each file of the tree in one journaled save.
///
/// # Errors
///
/// A file that cannot be staged or put in place, or an interrupted save that cannot be
/// recovered.
pub fn write_changed(root: &Path, changed: &Files) -> Result<(), StoreError> {
    journal::save(root, &tree_changes(changed))
}

fn tree_changes(files: &Files) -> Changes {
    Changes {
        write: files
            .iter()
            .map(|(path, bytes)| (RootPath::in_spec(path), bytes.clone()))
            .collect(),
        remove: BTreeSet::new(),
    }
}

/// Whether `path`, taken under `root` when relative, lies under `<root>/spec/` (RQ-171). The
/// path is judged by its components, `.` and `..` resolved, so `docs/../spec/x` is under it.
#[must_use]
pub fn is_under_spec(root: &Path, path: &Path) -> bool {
    let normal = |path: &Path| {
        let mut parts: Vec<std::path::Component<'_>> = Vec::new();
        for part in path.components() {
            match part {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    parts.pop();
                }
                other => parts.push(other),
            }
        }
        parts.into_iter().collect::<PathBuf>()
    };
    normal(&root.join(path)).starts_with(normal(&root.join(SPEC)))
}

/// Writes each output whose bytes differ from the file at its path, through a temporary file
/// beside it and a rename, and returns the paths written (AC-548). Relative paths are taken
/// under `root`.
///
/// # Errors
///
/// A file that cannot be read, written or renamed; outputs written before it stay written.
pub fn write_outputs(
    root: &Path,
    outputs: &[(PathBuf, Vec<u8>)],
) -> Result<Vec<PathBuf>, StoreError> {
    let mut written = Vec::new();
    for (path, bytes) in outputs {
        let target = root.join(path);
        if fs::read(&target).is_ok_and(|current| current == *bytes) {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io_error(parent))?;
        }
        let name = target
            .file_name()
            .map_or_else(Default::default, |name| name.to_string_lossy());
        let temporary = target.with_file_name(format!(".{name}.loft-{}", process::id()));
        fs::write(&temporary, bytes).map_err(io_error(&temporary))?;
        fs::rename(&temporary, &target).map_err(io_error(&target))?;
        written.push(path.clone());
    }
    Ok(written)
}

/// The file beside `spec/` that keeps the v1 facts the content view excludes (AC-508, AC-703).
pub const SIDECAR: &str = "v1-sidecar.json";

/// Writes an imported tree into an empty or missing `<root>/spec/`, and the sidecar beside it,
/// in one save.
///
/// # Errors
///
/// `spec/` holds a file or directory already (RQ-134), in which case nothing is written; or a
/// file cannot be written.
pub fn write_import(root: &Path, files: &Files, sidecar: &[u8]) -> Result<(), StoreError> {
    let spec = root.join(SPEC);
    if spec.exists() {
        for entry in fs::read_dir(&spec).map_err(io_error(&spec))? {
            let entry = entry.map_err(io_error(&spec))?;
            if !entry.file_name().to_str().is_some_and(hidden) {
                return Err(StoreError::NotEmpty(spec));
            }
        }
    }
    fs::create_dir_all(&spec).map_err(io_error(&spec))?;
    let mut changes = tree_changes(files);
    changes
        .write
        .insert(RootPath::beside(SIDECAR), sidecar.to_vec());
    journal::save(root, &changes)
}
