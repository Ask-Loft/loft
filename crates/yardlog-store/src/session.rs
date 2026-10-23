//! The files each session writes alone in Yard Log's folder, its token totals and its activity:
//! written whole to a temporary file beside the file and renamed into place, outside Yard Log's
//! lock, since only that session's events change them and they change no revision. And the
//! Yard Logs a worktree's `.yard/` holds, each branch's by its folder.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use loft_ports::io_error;
use loft_ports::sync::replace_synced;
use yardlog::codec::ROOT;
use yardlog::codec::session::{ACTIVITY, TOKENS};

use crate::files::YARD;
use crate::{YardStore, YardStoreError};

/// A file a session writes alone in Yard Log's folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionFile {
    /// The session's token totals, `tokens/<session id>.yaml`.
    Tokens,
    /// The session's activity, `activity/<session id>.yaml`.
    Activity,
}

impl SessionFile {
    #[must_use]
    pub const fn folder(self) -> &'static str {
        match self {
            Self::Tokens => TOKENS,
            Self::Activity => ACTIVITY,
        }
    }
}

/// Whether `session` can name a file: letters, digits, `.`, `_` and `-`, not starting with a
/// `.`, and at most 128 of them, so that no session id reaches outside its folder.
#[must_use]
pub fn is_session_id(session: &str) -> bool {
    !session.is_empty()
        && session.len() <= 128
        && !session.starts_with('.')
        && session
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

impl YardStore {
    /// The path of a session's file in Yard Log's folder.
    #[must_use]
    pub fn session_path(&self, file: SessionFile, session: &str) -> PathBuf {
        self.folder()
            .join(file.folder())
            .join(format!("{session}.yaml"))
    }

    /// Replaces a session's file with `bytes`, through a temporary file beside it renamed into
    /// place.
    ///
    /// # Errors
    ///
    /// `session` cannot name a file, or the file cannot be written.
    pub fn write_session(
        &self,
        file: SessionFile,
        session: &str,
        bytes: &[u8],
    ) -> Result<(), YardStoreError> {
        if !is_session_id(session) {
            return Err(YardStoreError::Session(session.to_owned()));
        }
        let path = self.session_path(file, session);
        let dir = path.parent().expect("a session's file lies in a folder");
        fs::create_dir_all(dir).map_err(io_error(dir))?;
        Ok(replace_synced(&path, bytes, false)?)
    }
}

/// The branches whose Yard Log the worktree whose top is `tree` holds under `.yard/`: each folder
/// that holds a root, by its path below `.yard/`, in order, and none within such a folder.
///
/// # Errors
///
/// A directory under `.yard/` cannot be read.
pub fn branches(tree: &Path) -> Result<Vec<String>, YardStoreError> {
    let mut found = Vec::new();
    walk(&tree.join(YARD), "", &mut found)?;
    found.sort();
    Ok(found)
}

fn walk(dir: &Path, rel: &str, found: &mut Vec<String>) -> Result<(), YardStoreError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(dir)(error).into()),
    };
    // A folder that holds a root is a branch's Yard Log, and nothing below it is another: git
    // allows no branch `a/b/c` beside a branch `a/b`.
    if !rel.is_empty() && dir.join(ROOT).is_file() {
        found.push(rel.to_owned());
        return Ok(());
    }
    for entry in entries {
        let entry = entry.map_err(io_error(dir))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        walk(&path, &rel, found)?;
    }
    Ok(())
}
