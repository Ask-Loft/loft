//! What belongs to the machine and not to a branch, in `~/.yard/` in the person's home
//! directory: the running page server, the trees it serves, when each session was last heard,
//! the revision each agent last read, the sessions that continue others, the worktrees each
//! session wrote Yard Log in, the directory of each session's main agent, the control token, and each session's events. Each file is written
//! whole to a temporary file beside it and renamed into place, so a reader never meets one half
//! written; a change that reads a file and writes it back holds the directory's lock while it
//! does.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use loft_ports::io_error;
use loft_ports::sync::{read_if_present, remove_if_present, replace_synced};

use crate::{YardStoreError, lock};

/// The directory under the person's home directory.
pub const HOME_DIR: &str = ".yard";

/// A file of `~/.yard/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeFile {
    /// The running page server's port, process id, version and formats.
    Server,
    /// The trees the page server serves.
    Trees,
    /// When each session was last heard.
    Sessions,
    /// The revision each agent last read.
    LastRead,
    /// The sessions that continue others.
    Continuations,
    /// The worktrees each session wrote Yard Log in.
    SessionTrees,
    /// The directory each session's main agent last named.
    SessionDirs,
    /// The page server's control token, readable only by the person's own account.
    Token,
    /// A newer page server's request that the running one stop.
    Stop,
}

impl HomeFile {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Server => "server.json",
            Self::Trees => "trees.json",
            Self::Sessions => "sessions.json",
            Self::LastRead => "last-read.json",
            Self::Continuations => "continuations.json",
            Self::SessionTrees => "session-trees.json",
            Self::SessionDirs => "session-dirs.json",
            Self::Token => "token",
            Self::Stop => "stop.json",
        }
    }

    /// Whether only the person's own account may read the file.
    const fn private(self) -> bool {
        matches!(self, Self::Token)
    }
}

/// `~/.yard/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    dir: PathBuf,
}

impl Home {
    /// `~/.yard/` in the home directory of the person who runs `loft`, where one is known.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        std::env::home_dir()
            .filter(|home| home.is_absolute())
            .map(|home| Self::at(home.join(HOME_DIR)))
    }

    /// The directory `dir` in place of `~/.yard/`.
    #[must_use]
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn path(&self, file: HomeFile) -> PathBuf {
        self.dir.join(file.name())
    }

    /// A session's event log, `events/<session id>.jsonl`.
    #[must_use]
    pub fn events(&self, session: &str) -> PathBuf {
        self.dir.join("events").join(format!("{session}.jsonl"))
    }

    /// A file's bytes, or none where it is not there.
    ///
    /// # Errors
    ///
    /// The file cannot be read.
    pub fn read(&self, file: HomeFile) -> Result<Option<Vec<u8>>, YardStoreError> {
        Ok(read_if_present(&self.path(file))?)
    }

    /// Replaces a file with `bytes`, through a temporary file beside it renamed into place.
    ///
    /// # Errors
    ///
    /// The directory or the file cannot be written.
    pub fn write(&self, file: HomeFile, bytes: &[u8]) -> Result<(), YardStoreError> {
        fs::create_dir_all(&self.dir).map_err(io_error(&self.dir))?;
        Ok(replace_synced(&self.path(file), bytes, file.private())?)
    }

    /// Removes a file, where it is there.
    ///
    /// # Errors
    ///
    /// The file is there and cannot be removed.
    pub fn remove(&self, file: HomeFile) -> Result<(), YardStoreError> {
        Ok(remove_if_present(&self.path(file))?)
    }

    /// The sessions whose event logs `events/` holds, by their ids, in order.
    ///
    /// # Errors
    ///
    /// The folder is there and cannot be read.
    pub fn event_sessions(&self) -> Result<Vec<String>, YardStoreError> {
        let dir = self.dir.join("events");
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io_error(&dir)(error).into()),
        };
        let mut sessions = Vec::new();
        for entry in entries {
            let path = entry.map_err(io_error(&dir))?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
                && let Some(session) = path.file_stem().and_then(|stem| stem.to_str())
            {
                sessions.push(session.to_owned());
            }
        }
        sessions.sort();
        Ok(sessions)
    }

    /// A session's event log, or none where it has none.
    ///
    /// # Errors
    ///
    /// The log is there and cannot be read.
    pub fn read_events(&self, session: &str) -> Result<Option<Vec<u8>>, YardStoreError> {
        Ok(read_if_present(&self.events(session))?)
    }

    /// Replaces a session's event log with `bytes`, through a temporary file beside it renamed
    /// into place. A page server appends to the log without a lock, so the caller holds the page
    /// server's lock, `_server`, which keeps any page server from starting meanwhile.
    ///
    /// # Errors
    ///
    /// The log cannot be written.
    pub fn replace_events(
        &self,
        _server: &lock::Locked,
        session: &str,
        bytes: &[u8],
    ) -> Result<(), YardStoreError> {
        Ok(replace_synced(&self.events(session), bytes, false)?)
    }

    /// Appends `line` and a line break to a session's event log, creating the log where it is
    /// not there.
    ///
    /// # Errors
    ///
    /// The log cannot be created or written.
    pub fn append_event(&self, session: &str, line: &[u8]) -> Result<(), YardStoreError> {
        let path = self.events(session);
        let dir = path.parent().expect("the event log lies in a folder");
        fs::create_dir_all(dir).map_err(io_error(dir))?;
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(io_error(&path))?;
        let mut whole = line.to_vec();
        whole.push(b'\n');
        log.write_all(&whole).map_err(io_error(&path))?;
        Ok(())
    }

    /// Reads a file and writes back what `change` makes of it, with no other change to the file
    /// between, under `~/.yard/`'s lock; `change` given `None` where the file is not there, and
    /// returning `None` to leave it as it is.
    ///
    /// # Errors
    ///
    /// The lock cannot be taken, or the file cannot be read or written.
    pub fn update(
        &self,
        file: HomeFile,
        change: impl FnOnce(Option<Vec<u8>>) -> Option<Vec<u8>>,
    ) -> Result<(), YardStoreError> {
        let _locked = lock::lock(&self.dir.join("lock"))?;
        if let Some(bytes) = change(self.read(file)?) {
            self.write(file, &bytes)?;
        }
        Ok(())
    }
}
