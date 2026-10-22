//! What belongs to the machine and not to a branch, in `~/.yard/` in the person's home
//! directory: the running page server, the trees it serves, when each session was last heard,
//! the revision each agent last read, the sessions that continue others, the control token, and
//! each session's events. Each file is written whole to a temporary file beside it and renamed
//! into place, so a reader never meets one half written; a change that reads a file and writes
//! it back holds the directory's lock while it does.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use loft_ports::io_error;
use loft_ports::sync::sync_dir;

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
    /// The page server's control token, readable only by the person's own account.
    Token,
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
            Self::Token => "token",
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
        let path = self.path(file);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(io_error(&path)(error).into()),
        }
    }

    /// Replaces a file with `bytes`, through a temporary file beside it renamed into place.
    ///
    /// # Errors
    ///
    /// The directory or the file cannot be written.
    pub fn write(&self, file: HomeFile, bytes: &[u8]) -> Result<(), YardStoreError> {
        fs::create_dir_all(&self.dir).map_err(io_error(&self.dir))?;
        let path = self.path(file);
        let temporary = self.dir.join(format!(".{}.{}", file.name(), unique()));
        let written = write_new(&temporary, bytes, file.private()).and_then(|()| {
            fs::rename(&temporary, &path)
                .map_err(io_error(&path))
                .map_err(Into::into)
        });
        if written.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        written?;
        Ok(sync_dir(&self.dir)?)
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

/// A name no other temporary file of this process or another has at once.
fn unique() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}.{}.tmp",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// Writes a new file, synced, readable only by its owner where `private`.
fn write_new(path: &Path, bytes: &[u8], private: bool) -> Result<(), YardStoreError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut handle = options.open(path).map_err(io_error(path))?;
    handle.write_all(bytes).map_err(io_error(path))?;
    handle.sync_all().map_err(io_error(path))?;
    Ok(())
}
