//! The states a repository has published, as its history holds them: each publish sequence as the
//! earliest commit that holds its lock holds it.

use std::collections::BTreeMap;
use std::path::Path;

use loftspec::codec::Files;
use loftspec::publish::{Lock, ReadTree, verify};
use loftspec_store::StoreError;
use loftspec_store::repository::GitCommand;

/// A published state as the history holds it: its lock, the lock's bytes and its files under
/// `spec/`.
pub struct Published {
    pub lock: Lock,
    pub bytes: Vec<u8>,
    pub files: Files,
}

impl Published {
    /// Whether its files verify against its lock.
    pub fn verifies(&self) -> bool {
        ReadTree::read(&self.files).is_ok_and(|tree| verify(&tree, &self.lock).is_ok())
    }
}

/// Each publish sequence the history of the repository at `root` holds, as the earliest commit
/// that holds its lock holds it. A commit whose lock does not read is skipped.
///
/// # Errors
///
/// git cannot list the commits that wrote the lock, or read one of them.
pub fn earliest(root: &Path) -> Result<BTreeMap<u64, Published>, StoreError> {
    let git = GitCommand::default();
    let mut earliest = BTreeMap::new();
    for commit in git.lock_history(root)? {
        let (Some(bytes), files) = git.lock_and_spec_at(root, &commit)? else {
            continue;
        };
        let Ok(lock) = Lock::read(&bytes) else {
            continue;
        };
        earliest
            .entry(lock.publish_sequence.get())
            .or_insert(Published { lock, bytes, files });
    }
    Ok(earliest)
}
