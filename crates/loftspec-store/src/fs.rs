//! The paths of the Format on disk, and the outputs a command writes outside the tree.

use std::path::{Path, PathBuf};
use std::{fs, process};

use crate::error::{StoreError, io_error};

/// The directory under the root that holds the tree.
pub const SPEC: &str = "spec";

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
