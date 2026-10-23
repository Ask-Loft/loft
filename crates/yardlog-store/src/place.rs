//! Where an agent's call reaches Yard Log: the worktree that holds the agent's directory, the
//! branch it is on, the repository's common directory, and the directory git keeps for the
//! worktree, under which Yard Log's locks, leases and journals lie. Git never checks one branch
//! out in two worktrees, so the tree and the branch name one branch's Yard Log.

use std::path::{Path, PathBuf};

use yardlog::SubmissionKey;

use crate::YardStoreError;
use crate::git::YardGit;
use crate::lock::{self, Locked};

/// The directory, under the worktree's own git directory, of Yard Log's locks, leases and
/// journals, as `git rev-parse --git-path` is given it.
pub const GIT_DIR_NAME: &str = "yard";

/// The worktree a call runs in, and what git says of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// The top of the worktree.
    pub tree: PathBuf,
    /// The branch the worktree is on, or none on a detached HEAD.
    pub branch: Option<String>,
    /// The repository's common directory, which every worktree of the repository shares.
    pub common_dir: PathBuf,
    /// `yard` under the worktree's own git directory.
    pub git_yard: PathBuf,
}

impl Place {
    /// The worktree that holds `dir`, the directory a call runs in.
    ///
    /// # Errors
    ///
    /// `dir` is in no repository, or git fails or cannot be run.
    pub fn locate(git: &YardGit, dir: &Path) -> Result<Self, YardStoreError> {
        // git prints its directories from the one it runs in, resolved on disk.
        let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_owned());
        let (tree, common_dir, git_yard) = git.locate(&dir)?;
        Ok(Self {
            branch: git.branch(&dir)?,
            common_dir,
            git_yard,
            tree,
        })
    }

    /// The branch, refusing a detached HEAD.
    ///
    /// # Errors
    ///
    /// The worktree is on a detached HEAD.
    pub fn branch(&self) -> Result<&str, YardStoreError> {
        self.branch
            .as_deref()
            .ok_or_else(|| YardStoreError::Detached(self.tree.clone()))
    }

    /// The file of the lease on a submission, under the worktree's git directory.
    #[must_use]
    pub fn lease_path(&self, submission: SubmissionKey) -> PathBuf {
        self.git_yard
            .join("leases")
            .join(format!("{submission}.lease"))
    }

    /// Takes the lease on a submission, for as long as the returned lock is held, or none where
    /// another wait holds it.
    ///
    /// # Errors
    ///
    /// The lease's file cannot be created, opened or locked.
    pub fn lease(&self, submission: SubmissionKey) -> Result<Option<Locked>, YardStoreError> {
        lock::try_lock(&self.lease_path(submission))
    }

    /// Whether a wait holds the lease on a submission now.
    ///
    /// # Errors
    ///
    /// The lease's file cannot be created, opened or locked.
    pub fn is_leased(&self, submission: SubmissionKey) -> Result<bool, YardStoreError> {
        lock::is_held(&self.lease_path(submission))
    }

    /// Takes the lease on a submission, as [`Self::lease`] does, and records `holder`, who the
    /// wait that holds it is, beside it.
    ///
    /// # Errors
    ///
    /// The lease's file cannot be locked, or its holder's file cannot be written.
    pub fn lease_as(
        &self,
        submission: SubmissionKey,
        holder: &[u8],
    ) -> Result<Option<Locked>, YardStoreError> {
        lock::try_lock_as(&self.lease_path(submission), holder)
    }

    /// Who holds the lease on a submission, as its holder recorded it, where a wait holds it.
    ///
    /// # Errors
    ///
    /// The lease's file cannot be locked, or its holder's file cannot be read.
    pub fn lease_holder(
        &self,
        submission: SubmissionKey,
    ) -> Result<Option<Vec<u8>>, YardStoreError> {
        lock::holder(&self.lease_path(submission))
    }
}
