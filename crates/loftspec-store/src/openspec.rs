//! Reading an `OpenSpec` directory into the files the `OpenSpec` converter takes: each `spec.md`
//! below `specs/`, never through a symbolic link, `project.md` and `config.yaml`, the active
//! changes and the number of archived ones. And writing an `OpenSpec` projection into a
//! directory: its main specs under each of its roots and its render map. A root that already
//! holds an entry is refused unless the caller replaces the main specs, which removes each
//! `spec.md` under the root that the projection does not write.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use loftspec::convert::openspec::Project;
use loftspec::render::openspec::{OpenspecProjection, RENDER_MAP};

use crate::StoreError;
use crate::error::io_error;
use crate::fs::{hidden, holds_entries, write_outputs};
use loftspec::convert::openspec::layout::{ARCHIVE, CHANGES, CONFIG, PROJECT_MD, SPEC_FILE, SPECS};

/// The files of the `OpenSpec` directory `dir` that the converter takes.
///
/// # Errors
///
/// `dir` cannot be read as a directory, or a file the converter takes cannot be read or is not
/// UTF-8.
pub fn read_project(dir: &Path) -> Result<Project, StoreError> {
    fs::read_dir(dir).map_err(io_error(dir))?;
    let mut specs = Vec::new();
    walk(&dir.join(SPECS), "", &mut specs)?;
    let changes = dir.join(CHANGES);
    Ok(Project {
        specs,
        project_md: optional(&dir.join(PROJECT_MD))?,
        config_yaml: optional(&dir.join(CONFIG))?,
        active_changes: directories(&changes)
            .filter(|name| name != ARCHIVE)
            .collect(),
        archived_changes: directories(&changes.join(ARCHIVE)).count()
            + directories(&dir.join(ARCHIVE)).count(),
    })
}

/// Each `spec.md` below `dir`, by its path below the walk's start, never through a symbolic
/// link.
fn walk(dir: &Path, prefix: &str, specs: &mut Vec<(String, String)>) -> Result<(), StoreError> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if kind.is_dir() {
            walk(&entry.path(), &format!("{path}/"), specs)?;
        } else if kind.is_file() && name == SPEC_FILE {
            specs.push((path, text(&entry.path())?));
        }
    }
    Ok(())
}

fn optional(path: &Path) -> Result<Option<String>, StoreError> {
    if path.is_file() {
        text(path).map(Some)
    } else {
        Ok(None)
    }
}

fn text(path: &Path) -> Result<String, StoreError> {
    let bytes = fs::read(path).map_err(io_error(path))?;
    String::from_utf8(bytes).map_err(|_| StoreError::NotText(path.to_path_buf()))
}

/// The names of the directories directly in `dir`, sorted; none when `dir` does not exist.
fn directories(dir: &Path) -> impl Iterator<Item = String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.into_iter()
}

/// What writing a projection did, each path inside the directory written to.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct WrittenProjection {
    pub written: Vec<PathBuf>,
    pub unchanged: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
}

/// Why a projection was not written.
#[derive(Debug, thiserror::Error)]
pub enum WriteProjectionError {
    #[error("{0}: the directory is not empty; give `--force` to replace its main specs")]
    NotEmpty(PathBuf),
    #[error("{0}: a symbolic link; the projection is never written through one")]
    Link(PathBuf),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Writes `projection` into `dir`. A path to a file it writes that passes a symbolic link below
/// `dir` is refused before anything is written, and so, without `replace`, is a root of the
/// projection that holds an entry whose name does not begin with `.`. With `replace`, once every
/// file is written, each `spec.md` under a root that the projection does not write is removed,
/// never below an entry whose name begins with `.`, and each directory that leaves empty.
///
/// # Errors
///
/// A root that is not empty without `replace`, a symbolic link on the way to a file, a root that
/// cannot be read, or a file that cannot be written or removed.
pub fn write_projection(
    dir: &Path,
    projection: OpenspecProjection,
    replace: bool,
) -> Result<WrittenProjection, WriteProjectionError> {
    let roots: Vec<PathBuf> = projection.roots.iter().map(PathBuf::from).collect();
    let mut outputs: Vec<(PathBuf, Vec<u8>)> = projection
        .files
        .into_iter()
        .map(|(path, text)| (PathBuf::from(path), text.into_bytes()))
        .collect();
    outputs.push((PathBuf::from(RENDER_MAP), projection.map.into_bytes()));
    let mut paths = roots.iter().chain(outputs.iter().map(|(path, _)| path));
    if let Some(link) = paths.find_map(|path| link_on(dir, path)) {
        return Err(WriteProjectionError::Link(link));
    }
    if !replace {
        for root in &roots {
            let root = dir.join(root);
            if root.exists() && holds_entries(&root)? {
                return Err(WriteProjectionError::NotEmpty(root));
            }
        }
    }
    let kept: BTreeSet<PathBuf> = outputs.iter().map(|(path, _)| dir.join(path)).collect();
    let outputs = write_outputs(dir, &outputs)?;
    let mut removed = Vec::new();
    if replace {
        for root in &roots {
            remove_others(&dir.join(root), &kept, &mut removed)?;
        }
    }
    let inside = |path: PathBuf| {
        path.strip_prefix(dir)
            .map_or_else(|_| path.clone(), Path::to_path_buf)
    };
    Ok(WrittenProjection {
        written: outputs.written,
        unchanged: outputs.unchanged,
        removed: removed.into_iter().map(inside).collect(),
    })
}

/// The first symbolic link on the way from `dir` to `path` below it, `path` included; none when
/// the way holds none or reaches a part that does not exist yet.
fn link_on(dir: &Path, path: &Path) -> Option<PathBuf> {
    let mut at = dir.to_path_buf();
    for part in path.components() {
        at.push(part);
        match fs::symlink_metadata(&at) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Some(at),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
}

/// Removes each `spec.md` below `dir` that is not `kept`, never through a symbolic link nor
/// below an entry whose name begins with `.`, and each directory below `dir` that leaves empty.
fn remove_others(
    dir: &Path,
    kept: &BTreeSet<PathBuf>,
    removed: &mut Vec<PathBuf>,
) -> Result<(), StoreError> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if entry.file_name().to_str().is_none_or(hidden) {
            continue;
        }
        let kind = entry.file_type().map_err(io_error(&path))?;
        if kind.is_dir() {
            let before = removed.len();
            remove_others(&path, kept, removed)?;
            let empty = fs::read_dir(&path)
                .map_err(io_error(&path))?
                .next()
                .is_none();
            if removed.len() > before && empty {
                fs::remove_dir(&path).map_err(io_error(&path))?;
            }
        } else if kind.is_file() && entry.file_name() == SPEC_FILE && !kept.contains(&path) {
            fs::remove_file(&path).map_err(io_error(&path))?;
            removed.push(path);
        }
    }
    Ok(())
}
