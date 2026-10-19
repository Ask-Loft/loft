//! The tree at the commit where HEAD left a base revision, read from git, for the commands that
//! compare the working tree with it. The base is `git merge-base HEAD <revision>`, so a fix made
//! on the revision's side after HEAD left it is not counted against HEAD; a revision HEAD
//! already contains is its own base.

use std::path::Path;

use loftspec::codec::Files;
use loftspec_store::StoreError;
use loftspec_store::repository::{GitCommand, MergeBase};

use crate::stop::Stop;

/// The tree at a base: the commit, and the files of `spec/` there.
pub struct Base {
    pub commit: String,
    pub files: Files,
}

/// The tree at the commit where HEAD left `revision`.
pub fn read(git: &GitCommand, root: &Path, revision: &str) -> Result<Base, Stop> {
    if let Err(error) = git.prefix(root) {
        return Err(match error {
            StoreError::NotARepository(root) => Stop::one(
                "base",
                format!(
                    "{}: not inside a git repository, which a base revision needs",
                    root.display()
                ),
            ),
            error => error.into(),
        });
    }
    let commit = match git.merge_base(root, revision) {
        Ok(MergeBase::Commit(commit)) => commit,
        Ok(MergeBase::Unrelated) => {
            let message = format!("`{revision}` shares no history with HEAD");
            return Err(Stop::one("base", message));
        }
        Err(StoreError::UnknownRevision(revision)) => {
            return Err(Stop::one("base", format!("`{revision}` names no commit")));
        }
        Err(error) => return Err(error.into()),
    };
    let files = git.spec_files_at(root, &commit)?;
    Ok(Base { commit, files })
}

/// The base's read errors, each naming the commit, as errors of kind `base`.
pub fn unread(commit: &str, errors: &[impl std::fmt::Display]) -> Stop {
    let messages = errors
        .iter()
        .map(|error| format!("the tree at {commit} does not read: {error}"))
        .collect();
    Stop::all("base", messages)
}
