//! What the publisher needs to know of the repository that holds the tree: the branch, the
//! uncommitted changes under `spec/`, and a merge or rebase stopped on a conflict.

/// What the publisher needs to know of the repository that holds the tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoState {
    /// The checked-out branch, or none on a detached HEAD.
    pub branch: Option<String>,
    /// The paths under `spec/` with uncommitted changes, from the directory that holds `spec/`,
    /// sorted and without duplicates.
    pub modified: Vec<String>,
    /// A merge or a rebase that stopped on a conflict, where one did.
    pub stopped: Option<Stopped>,
}

/// What stopped: `git merge`, or `git rebase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Merge,
    Rebase,
}

impl Operation {
    /// The reference that names the commit being merged or replayed.
    #[must_use]
    pub const fn other_head(self) -> &'static str {
        match self {
            Self::Merge => "MERGE_HEAD",
            Self::Rebase => "REBASE_HEAD",
        }
    }
}

/// A merge or a rebase that stopped on a conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stopped {
    pub operation: Operation,
    /// The branch the operation completes: the checked-out branch of a merge, or the branch a
    /// rebase rebases; none for a rebase of a detached HEAD.
    pub branch: Option<String>,
    /// The bytes of the lock beside `spec/` at `HEAD`, where it holds one.
    pub head_lock: Option<Vec<u8>>,
    /// The bytes of the lock at the commit being merged or replayed, where it holds one.
    pub other_lock: Option<Vec<u8>>,
    /// The unmerged paths, sorted and without duplicates: from the directory that holds `spec/`
    /// where they are under it, and otherwise from the top level after `:/`.
    pub unmerged: Vec<String>,
}
