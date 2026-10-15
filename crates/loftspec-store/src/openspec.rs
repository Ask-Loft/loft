//! Reading an `OpenSpec` directory into the files the `OpenSpec` converter takes: each `spec.md`
//! below `specs/`, never through a symbolic link, `project.md` and `config.yaml`, the active
//! changes and the number of archived ones.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use loftspec::convert::openspec::Project;
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
