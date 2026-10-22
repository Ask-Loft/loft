//! What can go wrong in a port: a file, a journal, git or the clock.

use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PortError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{0}: a save is already in progress")]
    Pending(PathBuf),
    #[error("{0}: {1}")]
    Journal(PathBuf, String),
    #[error("git cannot be run: {0}")]
    GitUnavailable(String),
    #[error("`git {call}` failed: {message}")]
    Git { call: String, message: String },
    #[error("SOURCE_DATE_EPOCH is {0:?}, not a whole number of seconds")]
    Epoch(String),
}

/// Names `path` in an I/O error.
pub fn io_error(path: &Path) -> impl FnOnce(io::Error) -> PortError + '_ {
    move |source| PortError::Io {
        path: path.to_path_buf(),
        source,
    }
}
