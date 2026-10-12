//! What can go wrong in a store: the document's own errors, and those of the files that hold it.

use std::io;
use std::path::{Path, PathBuf};

use loftspec::codec::ReadError;
use loftspec::index::IndexErrors;
use loftspec::publish::LockError;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("the store holds no document")]
    NoDocument,
    #[error("{}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"))]
    Read(Vec<ReadError>),
    #[error("the document does not index: {0:?}")]
    Index(IndexErrors),
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
    #[error("{0}: `spec/` is not empty")]
    NotEmpty(PathBuf),
    #[error("{0}: a save is already in progress")]
    Pending(PathBuf),
    #[error("{0}: {1}")]
    Journal(PathBuf, String),
    #[error("{0}: not inside a git repository; a publish needs one")]
    NotARepository(PathBuf),
    #[error("git cannot be run: {0}")]
    GitUnavailable(String),
    #[error("`git {call}` failed: {message}")]
    Git { call: String, message: String },
    #[error("SOURCE_DATE_EPOCH is {0:?}, not a whole number of seconds")]
    Epoch(String),
    #[error("{0}: not a valid lock record: {1}")]
    Lock(PathBuf, LockError),
}

pub(crate) fn io_error(path: &Path) -> impl FnOnce(io::Error) -> StoreError + '_ {
    move |source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}
