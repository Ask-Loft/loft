//! The publisher (CMP-9): the preconditions of a publish, label allocation, the lock and its
//! verification. It decides from the tree, the repository's state and the previous lock it is
//! given; reading the repository and writing files belong to a store.

mod lock;
mod time;

pub use lock::{LOCK, Lock, LockError};
pub use time::{PublishedAt, TimeError};

/// What the publisher needs to know of the repository that holds the tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoState {
    /// The checked-out branch, or none on a detached HEAD.
    pub branch: Option<String>,
    /// The paths under `spec/` with uncommitted changes, from the directory that holds `spec/`,
    /// sorted and without duplicates.
    pub modified: Vec<String>,
}
