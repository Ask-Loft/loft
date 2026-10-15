//! Reading an `OpenSpec` directory into the files the `OpenSpec` converter takes: each `spec.md`
//! below `specs/`, never through a symbolic link, `project.md` and `config.yaml`, the active
//! changes and the number of archived ones. And writing an `OpenSpec` projection into a
//! directory: its main specs under each of its roots and its render map. A root that already
//! holds an entry is refused unless the caller replaces the main specs, which removes each
//! `spec.md` under the root that the projection does not write.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use loftspec::convert::openspec::Project;
use loftspec::render::openspec::{OpenspecProjection, RENDER_MAP};

use crate::StoreError;
use crate::fs::{hidden, write_outputs};
use loftspec::convert::openspec::layout::{ARCHIVE, CHANGES, CONFIG, PROJECT_MD, SPEC_FILE, SPECS};

/// A directory or a file of the project that cannot be read as the converter needs it.
#[derive(Debug, thiserror::Error)]
pub enum ReadProjectError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{0}: the bytes are not UTF-8")]
    NotText(PathBuf),
}

/// The files of the `OpenSpec` directory `dir` that the converter takes.
///
/// # Errors
///
/// `dir` cannot be read as a directory, or a file the converter takes cannot be read or is not
/// UTF-8.
pub fn read_project(dir: &Path) -> Result<Project, ReadProjectError> {
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
fn walk(
    dir: &Path,
    prefix: &str,
    specs: &mut Vec<(String, String)>,
) -> Result<(), ReadProjectError> {
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

fn optional(path: &Path) -> Result<Option<String>, ReadProjectError> {
    if path.is_file() {
        text(path).map(Some)
    } else {
        Ok(None)
    }
}

fn text(path: &Path) -> Result<String, ReadProjectError> {
    let bytes = fs::read(path).map_err(io_error(path))?;
    String::from_utf8(bytes).map_err(|_| ReadProjectError::NotText(path.to_path_buf()))
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

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> ReadProjectError + '_ {
    move |source| ReadProjectError::Io {
        path: path.to_path_buf(),
        source,
    }
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
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Writes `projection` into `dir`. Without `replace`, a root of the projection that holds an
/// entry whose name does not begin with `.` is refused before anything is written; with it,
/// each `spec.md` under a root that the projection does not write is removed, and each
/// directory that leaves empty.
///
/// # Errors
///
/// A root that is not empty without `replace`, or a file that cannot be written or removed.
pub fn write_projection(
    dir: &Path,
    projection: &OpenspecProjection,
    replace: bool,
) -> Result<WrittenProjection, WriteProjectionError> {
    let roots: Vec<PathBuf> = projection.roots.iter().map(|root| dir.join(root)).collect();
    if !replace && let Some(root) = roots.iter().find(|root| !is_empty(root)) {
        return Err(WriteProjectionError::NotEmpty(root.clone()));
    }
    let mut outputs: Vec<(PathBuf, Vec<u8>)> = projection
        .files
        .iter()
        .map(|(path, text)| (PathBuf::from(path), text.clone().into_bytes()))
        .collect();
    outputs.push((
        PathBuf::from(RENDER_MAP),
        projection.map.clone().into_bytes(),
    ));
    let kept: BTreeSet<PathBuf> = outputs.iter().map(|(path, _)| dir.join(path)).collect();
    let mut removed = Vec::new();
    for root in &roots {
        remove_others(root, &kept, &mut removed).map_err(crate::error::io_error(root))?;
    }
    let written = write_outputs(dir, &outputs)?;
    let unchanged = outputs
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| !written.contains(path))
        .collect();
    let inside = |path: PathBuf| {
        path.strip_prefix(dir)
            .map_or(path.clone(), Path::to_path_buf)
    };
    Ok(WrittenProjection {
        written,
        unchanged,
        removed: removed.into_iter().map(inside).collect(),
    })
}

/// Whether `dir` holds no entry whose name does not begin with `.`; a directory that does not
/// exist is empty.
fn is_empty(dir: &Path) -> bool {
    fs::read_dir(dir).map_or(true, |entries| {
        entries
            .flatten()
            .all(|entry| hidden(&entry.file_name().to_string_lossy()))
    })
}

/// Removes each `spec.md` below `dir` that is not `kept`, never through a symbolic link, and
/// each directory below `dir` that leaves empty.
fn remove_others(
    dir: &Path,
    kept: &BTreeSet<PathBuf>,
    removed: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let kind = entry.file_type()?;
        let path = entry.path();
        if kind.is_dir() {
            let before = removed.len();
            remove_others(&path, kept, removed)?;
            if removed.len() > before && fs::read_dir(&path)?.next().is_none() {
                fs::remove_dir(&path)?;
            }
        } else if kind.is_file() && entry.file_name() == SPEC_FILE && !kept.contains(&path) {
            fs::remove_file(&path)?;
            removed.push(path);
        }
    }
    Ok(())
}
