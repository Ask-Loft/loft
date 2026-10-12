//! The publisher (CMP-9): the preconditions of a publish, label allocation, the lock and its
//! verification. It decides from the tree, the repository's state and the previous lock it is
//! given; reading the repository and writing files belong to a store.

mod labels;
mod lock;
mod time;

use std::fmt;
use std::num::NonZeroU64;

pub use labels::{Allocated, allocate};
pub use lock::{LOCK, Lock, LockError};
pub use time::{PublishedAt, TimeError};

use crate::canonical::Digest;
use crate::check::{Placed, place};
use crate::codec::{self, Files, LABELS, ReadError, Sites, SpecPath, read_sited};
use crate::document::Document;
use crate::hash::{Roots, hash_document};
use crate::validate::validate;

/// The publish branch of a manifest that names none (AC-449).
pub const DEFAULT_PUBLISH_BRANCH: &str = "main";

/// What the publisher needs to know of the repository that holds the tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoState {
    /// The checked-out branch, or none on a detached HEAD.
    pub branch: Option<String>,
    /// The paths under `spec/` with uncommitted changes, from the directory that holds `spec/`,
    /// sorted and without duplicates.
    pub modified: Vec<String>,
}

/// What a publish decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Publication {
    /// Nothing changed since the previous lock, which stands (RQ-117).
    Unchanged(Lock),
    /// A new lock, the bytes of `spec/labels.yaml`, the only file under `spec/` a publish
    /// changes (AC-460), and the labels it allocated.
    Published {
        lock: Lock,
        labels: Vec<u8>,
        allocated: Vec<Allocated>,
    },
}

/// Why a publish refused before it wrote anything (F-27).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The tree does not read.
    Unread(Vec<ReadError>),
    /// Paths under `spec/` have uncommitted changes (RQ-112).
    Modified(Vec<String>),
    /// The current branch, none on a detached HEAD, is not the publish branch (RQ-113).
    Branch {
        current: Option<String>,
        publish: String,
    },
    /// The validator reports errors (RQ-114).
    Invalid(Vec<Placed>),
}

/// Publishes the tree: refuses on a read error, an uncommitted change under `spec/`, a branch
/// other than the publish branch or a validation error, in that order; reports the previous lock
/// when the tree's state root equals it; and otherwise allocates labels and gives the new lock
/// and bindings to write.
///
/// # Errors
///
/// A refusal, which writes nothing.
pub fn publish(
    files: &Files,
    repo: &RepoState,
    previous: Option<&Lock>,
    at: PublishedAt,
) -> Result<Publication, Refusal> {
    let (mut document, sites) = read_sited(files).map_err(Refusal::Unread)?;
    if !repo.modified.is_empty() {
        return Err(Refusal::Modified(repo.modified.clone()));
    }
    let branch = publish_branch(&document);
    if repo.branch.as_deref() != Some(branch) {
        return Err(Refusal::Branch {
            current: repo.branch.clone(),
            publish: branch.to_owned(),
        });
    }
    let roots = valid_roots(&document, &sites).map_err(Refusal::Invalid)?;
    if let Some(previous) = previous.filter(|lock| lock.state_root == roots.state) {
        return Ok(Publication::Unchanged(previous.clone()));
    }
    let allocated = allocate(&mut document);
    let roots = valid_roots(&document, &sites).map_err(Refusal::Invalid)?;
    let lock = Lock {
        document_key: document.id,
        state_root: roots.state,
        content_root: roots.content,
        binding_root: roots.bindings,
        publish_sequence: previous.map_or(NonZeroU64::MIN, |lock| {
            lock.publish_sequence.saturating_add(1)
        }),
        published_at: at,
        derived_from: document.derived_from.clone(),
    };
    let labels = codec::write(&document)
        .remove(&SpecPath::new(LABELS))
        .expect("the writer writes the bindings");
    Ok(Publication::Published {
        lock,
        labels,
        allocated,
    })
}

/// The branch a publish must run on: the manifest's `publishBranch`, or `main`.
#[must_use]
pub fn publish_branch(document: &Document) -> &str {
    document
        .publish_branch
        .as_deref()
        .unwrap_or(DEFAULT_PUBLISH_BRANCH)
}

/// The roots of a document the validator accepts, or its errors at their sites.
fn valid_roots(document: &Document, sites: &Sites) -> Result<Roots, Vec<Placed>> {
    let invalid: Vec<Placed> = validate(document)
        .into_iter()
        .map(|error| place(error, sites))
        .collect();
    if !invalid.is_empty() {
        return Err(invalid);
    }
    Ok(hash_document(document)
        .expect("a document the validator accepts indexes")
        .roots)
}

/// A root a lock records, in the order verification compares them (AC-678).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Root {
    Content,
    Binding,
    State,
}

impl Root {
    /// The lock field that records the root.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::Content => "contentRoot",
            Self::Binding => "bindingRoot",
            Self::State => "stateRoot",
        }
    }
}

impl fmt::Display for Root {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Content => "content root",
            Self::Binding => "binding root",
            Self::State => "state root",
        })
    }
}

/// Why a tree does not verify against a lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unverified {
    /// The tree does not read; no root is compared (RQ-149).
    Unread(Vec<ReadError>),
    /// The tree reads but its roots cannot be computed.
    Invalid(Vec<Placed>),
    /// The first root that differs (AC-469), with the lock's value and the tree's.
    Mismatch {
        root: Root,
        lock: Digest,
        tree: Digest,
    },
}

/// Recomputes the roots of the tree and compares them with the lock's: the content root, then
/// the binding root, then the state root (RQ-118, AC-678). It writes nothing (AC-470).
///
/// # Errors
///
/// The tree does not read, or a root differs.
pub fn verify(files: &Files, lock: &Lock) -> Result<Digest, Unverified> {
    let (document, sites) = read_sited(files).map_err(Unverified::Unread)?;
    let Ok(hashes) = hash_document(&document) else {
        let invalid = validate(&document)
            .into_iter()
            .map(|error| place(error, &sites))
            .collect();
        return Err(Unverified::Invalid(invalid));
    };
    let roots = hashes.roots;
    for (root, lock, tree) in [
        (Root::Content, lock.content_root, roots.content),
        (Root::Binding, lock.binding_root, roots.bindings),
        (Root::State, lock.state_root, roots.state),
    ] {
        if lock != tree {
            return Err(Unverified::Mismatch { root, lock, tree });
        }
    }
    Ok(roots.state)
}
