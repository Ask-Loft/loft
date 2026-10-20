//! Publishing a tree that reads, as the publisher's tests do: in a clean repository, at a fixed
//! time, and the tree with what the publish wrote.

use loftspec::codec::Files;
use loftspec::publish::{
    Lock, Made, Publication, PublishedAt, ReadTree, Refusal, RepoState, publish,
};

/// A repository on `branch` with no uncommitted change and nothing stopped.
pub fn clean_on(branch: &str) -> RepoState {
    RepoState {
        branch: Some(branch.to_owned()),
        modified: Vec::new(),
        stopped: None,
    }
}

/// A repository on `main` with no uncommitted change and nothing stopped.
pub fn clean_main() -> RepoState {
    clean_on("main")
}

/// The publish time `seconds` after the epoch.
pub fn at(seconds: u64) -> PublishedAt {
    PublishedAt::from_unix_seconds(seconds).unwrap()
}

/// Publishes a tree that reads, not as its remote.
pub fn publish_files(
    files: &Files,
    repo: &RepoState,
    previous: Option<&Lock>,
    at: PublishedAt,
) -> Result<Publication, Refusal> {
    publish(
        ReadTree::read(files).unwrap(),
        Some(repo),
        previous,
        at,
        Made::NotAsRemote,
    )
}

/// Publishes `files` after `previous` on a clean `main` at 1 000 seconds: the publication, and
/// the tree with what it wrote.
pub fn publish_after(files: &Files, previous: Option<&Lock>) -> (Publication, Files) {
    let publication = publish_files(files, &clean_main(), previous, at(1_000)).unwrap();
    let mut tree = files.clone();
    if let Publication::Published { files: written, .. } = &publication {
        tree.extend(written.clone());
    }
    (publication, tree)
}

/// The tree after a first publish of `files`, which must publish.
pub fn published(files: &Files) -> Files {
    let (publication, tree) = publish_after(files, None);
    assert!(
        matches!(publication, Publication::Published { .. }),
        "the tree is published"
    );
    tree
}
