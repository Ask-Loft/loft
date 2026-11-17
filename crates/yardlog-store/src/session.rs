//! The files each session writes alone for a branch's Yard Log, its token totals, its activity
//! and what its agents consulted. Each event of the session writes the file's live copy in git's
//! own directory for the worktree, whole to a temporary file beside it renamed into place, outside
//! Yard Log's lock, since only that session's events change it and they change no revision. Each
//! write of Yard Log saves the live copies that differ into Yard Log's folder through its journal,
//! so the working tree changes only when Yard Log is written. And the Yard Logs a worktree's
//! `.yard/` holds, each branch's by its folder.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use loft_ports::io_error;
use loft_ports::journal::{Changes, RelPath};
use loft_ports::sync::{read_if_present, replace_synced};
use yardlog::codec::ROOT;
use yardlog::codec::session::{ACTIVITY, CONSULTATION, TOKENS, writes_over};
use yardlog::identity::SessionKey;

use crate::files::YARD;
use crate::store::file_name;
use crate::{YardStore, YardStoreError};

/// A file a session writes alone in Yard Log's folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionFile {
    /// The session's token totals, `tokens/<session id>.yaml`.
    Tokens,
    /// The session's activity, `activity/<session id>.yaml`.
    Activity,
    /// What the session's agents consulted, `consultation/<session id>.yaml`.
    Consultation,
}

impl SessionFile {
    /// Every file a session writes alone.
    pub const ALL: [Self; 3] = [Self::Tokens, Self::Activity, Self::Consultation];

    #[must_use]
    pub const fn folder(self) -> &'static str {
        match self {
            Self::Tokens => TOKENS,
            Self::Activity => ACTIVITY,
            Self::Consultation => CONSULTATION,
        }
    }

    /// Session `session`'s file of this kind, by its path from Yard Log's folder or from the
    /// directory of the live copies.
    #[must_use]
    pub fn name(self, session: &str) -> String {
        format!("{}/{session}.yaml", self.folder())
    }
}

pub use yardlog::identity::is_session_id;

/// The directory, under `git_yard`, `yard` in a worktree's own git directory, of the live copies
/// of the session files of `branch`'s Yard Log, named by the branch as its lock is.
#[must_use]
pub fn live_dir(git_yard: &Path, branch: &str) -> PathBuf {
    git_yard.join(format!("{}.sessions", file_name(branch)))
}

impl YardStore {
    /// The path of a session's file in Yard Log's folder.
    #[must_use]
    pub fn session_path(&self, file: SessionFile, session: &str) -> PathBuf {
        self.folder().join(file.name(session))
    }

    /// The directory of the live copies of this Yard Log's session files.
    #[must_use]
    pub fn live_dir(&self) -> PathBuf {
        live_dir(&self.place().git_yard, self.branch())
    }

    /// The path of the live copy of a session's file.
    #[must_use]
    pub fn live_session_path(&self, file: SessionFile, session: &str) -> PathBuf {
        self.live_dir().join(file.name(session))
    }

    /// Replaces the live copy of a session's file with `bytes`, through a temporary file beside
    /// it renamed into place.
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
        checked(session)?;
        let path = self.live_session_path(file, session);
        let dir = path.parent().expect("a session's file lies in a folder");
        fs::create_dir_all(dir).map_err(io_error(dir))?;
        Ok(replace_synced(&path, bytes, false)?)
    }

    /// Rewrites a session's file where a read takes it from: its live copy where there is one,
    /// as [`Self::write_session`] does, else its copy in Yard Log's folder, saved through the
    /// journal under Yard Log's lock, as a command that upgrades the files does outside the
    /// session's events.
    ///
    /// # Errors
    ///
    /// `session` cannot name a file, the lock cannot be taken, or the file cannot be written.
    pub fn rewrite_session(
        &self,
        file: SessionFile,
        session: &str,
        bytes: &[u8],
    ) -> Result<(), YardStoreError> {
        checked(session)?;
        if self.live_session_path(file, session).is_file() {
            return self.write_session(file, session, bytes);
        }
        let _locked = self.locked()?;
        let mut changes = Changes::default();
        changes
            .write
            .insert(self.folder_file(file, session), bytes.to_vec());
        Ok(self.journal().save(&changes)?)
    }

    /// Session `session`'s file of `kind`: its live copy where there is one, else the copy in
    /// Yard Log's folder; none where neither can be read.
    #[must_use]
    pub fn session_file(&self, kind: SessionFile, session: &str) -> Option<Vec<u8>> {
        fs::read(self.live_session_path(kind, session))
            .or_else(|_| fs::read(self.session_path(kind, session)))
            .ok()
    }

    /// Every session's file of `kind`, by the session key that names it, in the order of their
    /// keys: each one's live copy where there is one, else its copy in Yard Log's folder. A file
    /// that cannot be read is left out, since only its own session's events change it and the
    /// next event writes it whole again.
    #[must_use]
    pub fn session_files(&self, kind: SessionFile) -> Vec<(String, Vec<u8>)> {
        let mut files = listed(&self.folder().join(kind.folder()));
        files.extend(listed(&self.live_dir().join(kind.folder())));
        files.into_iter().collect()
    }

    /// Adds to `changes` each live copy whose bytes differ from its copy in Yard Log's folder,
    /// leaving a copy of a format this `loft` does not write as it is.
    pub(crate) fn save_live(&self, changes: &mut Changes) -> Result<(), YardStoreError> {
        let live = SessionFile::ALL.into_iter().flat_map(|kind| {
            let files = listed(&self.live_dir().join(kind.folder()));
            files
                .into_iter()
                .map(move |(session, bytes)| (kind, session, bytes))
        });
        for (kind, session, bytes) in live {
            let folder = read_if_present(&self.session_path(kind, &session))?;
            if saves_over(kind, folder.as_deref(), &bytes) {
                changes
                    .write
                    .insert(self.folder_file(kind, &session), bytes);
            }
        }
        Ok(())
    }

    /// A session's file in Yard Log's folder, by its path from the top of the worktree.
    fn folder_file(&self, kind: SessionFile, session: &str) -> RelPath {
        RelPath::new(format!("{}/{}", self.folder_rel(), kind.name(session)))
    }
}

/// Refuses a session id that cannot name a file.
fn checked(session: &str) -> Result<(), YardStoreError> {
    if SessionKey::parse(session).is_valid() {
        Ok(())
    } else {
        Err(YardStoreError::Session(session.to_owned()))
    }
}

/// Whether a live copy of `kind` that holds `live` is saved over the copy in Yard Log's folder that
/// holds `folder`: where the folder holds none, or one that differs and is of a format this `loft`
/// writes.
fn saves_over(kind: SessionFile, folder: Option<&[u8]>, live: &[u8]) -> bool {
    folder.is_none_or(|folder| folder != live && writes_over(kind.folder(), folder))
}

/// Every `.yaml` file in `dir`, by its name without the extension, with its bytes; a file that
/// cannot be read is left out, and so is every file where `dir` cannot be read.
fn listed(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let Ok(entries) = fs::read_dir(dir) else {
        return BTreeMap::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yaml")
        })
        .filter_map(|path| {
            let key = path.file_stem()?.to_str()?.to_owned();
            Some((key, fs::read(&path).ok()?))
        })
        .collect()
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
