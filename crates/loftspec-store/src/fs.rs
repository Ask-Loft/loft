//! The `spec/` tree on disk: read whole, and written back file by file through a temporary
//! directory and a rename (AC-537), so a crash leaves each file either as it was or as written.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{fs, io, process};

use loftspec::codec::{Files, SpecPath};

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
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> StoreError + '_ {
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

fn collect(dir: &Path, prefix: &str, files: &mut Files) -> Result<(), StoreError> {
    for entry in fs::read_dir(dir).map_err(io_error(dir))? {
        let entry = entry.map_err(io_error(dir))?;
        let path = entry.path();
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| StoreError::NotUtf8(path.clone()))?;
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

/// Writes each file into a temporary directory beside `spec/`, then renames each into place.
///
/// # Errors
///
/// A file that cannot be written or renamed; files renamed before it stay written.
pub fn write_changed(root: &Path, changed: &Files) -> Result<(), StoreError> {
    if changed.is_empty() {
        return Ok(());
    }
    let staging = root.join(format!(".loft-write-{}", process::id()));
    fs::create_dir_all(&staging).map_err(io_error(&staging))?;
    let result = stage_and_rename(root, &staging, changed);
    let _ = fs::remove_dir_all(&staging);
    result
}

fn stage_and_rename(root: &Path, staging: &Path, changed: &Files) -> Result<(), StoreError> {
    let mut staged = Vec::with_capacity(changed.len());
    for (index, (path, bytes)) in changed.iter().enumerate() {
        let temporary = staging.join(index.to_string());
        fs::write(&temporary, bytes).map_err(io_error(&temporary))?;
        staged.push((temporary, root.join(SPEC).join(path.as_str())));
    }
    for (temporary, target) in staged {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io_error(parent))?;
        }
        fs::rename(&temporary, &target).map_err(io_error(&target))?;
    }
    Ok(())
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
