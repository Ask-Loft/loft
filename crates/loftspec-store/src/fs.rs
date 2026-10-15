//! The paths of the Format on disk, and the outputs a command writes outside the tree.

use std::path::{Component, Path, PathBuf};
use std::{fs, process};

use crate::error::{StoreError, io_error};

/// The directory under the root that holds the tree.
pub const SPEC: &str = "spec";

/// Whether `path`, taken under `root` when relative, lies under `<root>/spec/` (RQ-171). Both
/// are made absolute with `.` and `..` resolved, and the part of each that exists is resolved on
/// disk, so an absolute path, a symbolic link or, on a file system that ignores case, another
/// letter case cannot reach under `spec/` unseen.
#[must_use]
pub fn is_under_spec(root: &Path, path: &Path) -> bool {
    resolved(&root.join(path)).starts_with(resolved(&root.join(SPEC)))
}

/// `path` made absolute, `.` and `..` resolved, with its longest existing part resolved on disk.
fn resolved(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut normal = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                normal.pop();
            }
            other => normal.push(other),
        }
    }
    let mut missing = Vec::new();
    let mut existing = normal.as_path();
    while !existing.exists() {
        let (Some(name), Some(parent)) = (existing.file_name(), existing.parent()) else {
            break;
        };
        missing.push(name);
        existing = parent;
    }
    let mut resolved = existing
        .canonicalize()
        .unwrap_or_else(|_| existing.to_path_buf());
    resolved.extend(missing.iter().rev());
    resolved
}

/// What [`write_outputs`] did with each output: wrote it, or left it as the file already held
/// its bytes.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct WrittenOutputs {
    pub written: Vec<PathBuf>,
    pub unchanged: Vec<PathBuf>,
}

/// Writes each output whose bytes differ from the file at its path, through a temporary file
/// beside it and a rename (AC-548). Relative paths are taken under `root`.
///
/// # Errors
///
/// A file that cannot be read, written or renamed; outputs written before it stay written.
pub fn write_outputs(
    root: &Path,
    outputs: &[(PathBuf, Vec<u8>)],
) -> Result<WrittenOutputs, StoreError> {
    let mut done = WrittenOutputs::default();
    for (path, bytes) in outputs {
        let target = root.join(path);
        if fs::read(&target).is_ok_and(|current| current == *bytes) {
            done.unchanged.push(path.clone());
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
        done.written.push(path.clone());
    }
    Ok(done)
}

/// Whether `dir` holds an entry that is not [`hidden`].
///
/// # Errors
///
/// `dir` or one of its entries cannot be read.
pub(crate) fn holds_entries(dir: &Path) -> Result<bool, StoreError> {
    for entry in fs::read_dir(dir).map_err(io_error(dir))? {
        let entry = entry.map_err(io_error(dir))?;
        if !entry.file_name().to_str().is_some_and(hidden) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether an entry under `spec/` is left out of the tree: its name begins with `.`, as an
/// operating system's or an editor's own files do (`.DS_Store`, swap files).
pub(crate) fn hidden(name: &str) -> bool {
    name.starts_with('.')
}
