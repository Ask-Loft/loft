//! What can go wrong in the store of Yard Log.

use std::path::PathBuf;

use loft_ports::PortError;
use yardlog::Format;
use yardlog::codec::FileError;

#[derive(Debug, thiserror::Error)]
pub enum YardStoreError {
    #[error("{0}: not inside a git repository; Yard Log needs one")]
    NotARepository(PathBuf),
    #[error("{0}: the worktree is on a detached HEAD; Yard Log belongs to a branch")]
    Detached(PathBuf),
    #[error("`{0}` cannot name a folder of Yard Log")]
    Branch(String),
    #[error("`{0}` cannot name a session's file")]
    Session(String),
    /// A file of Yard Log does not read: its path from the top of the worktree, and why, at its
    /// line where the reader knows it.
    #[error("{folder}/{error}")]
    Unreadable { folder: String, error: FileError },
    #[error(
        "Yard Log is written in {found}, which is newer than {writes}, the newest this `loft` writes; a newer `loft` is needed to change it"
    )]
    Newer { found: Format, writes: Format },
    #[error("git answered `{call}` with {answer:?}, which the store does not read")]
    Answer { call: String, answer: String },
    #[error(transparent)]
    Port(#[from] PortError),
}
