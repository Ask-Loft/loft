//! The publisher (CMP-9): the preconditions of a publish, label allocation, the lock and its
//! verification. It decides from the tree, the repository's state and the previous lock it is
//! given; reading the repository and writing files belong to a store.

mod labels;
mod lock;
pub mod rewrite;
mod time;

use std::fmt;
use std::num::NonZeroU64;

pub use labels::{Allocated, allocate};
pub use lock::{LOCK, Lock, LockError};
pub use time::{PublishedAt, TimeError};

use rewrite::Rewritten;

use crate::canonical::Digest;
use crate::check::{Placed, placed};
use crate::codec::{self, Files, LABELS, ReadError, Sites, SpecPath, read_sited};
use crate::document::Document;
use crate::hash::hash_document;
use crate::validate::{conflict_errors, index_errors, validate};

/// What the publisher needs to know of the repository that holds the tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoState {
    /// The checked-out branch, or none on a detached HEAD.
    pub branch: Option<String>,
    /// The paths under `spec/` with uncommitted changes, from the directory that holds `spec/`,
    /// sorted and without duplicates.
    pub modified: Vec<String>,
}

/// A tree read for a publish or a verification: its files, its document and where each of its
/// elements sits. Reading it first is what makes a read error come before every other answer.
pub struct ReadTree<'a> {
    files: &'a Files,
    document: Document,
    sites: Sites,
}

impl<'a> ReadTree<'a> {
    /// Reads a `spec/` tree.
    ///
    /// # Errors
    ///
    /// Every read error the tree holds.
    pub fn read(files: &'a Files) -> Result<Self, Vec<ReadError>> {
        let (document, sites) = read_sited(files)?;
        Ok(Self {
            files,
            document,
            sites,
        })
    }
}

/// What a publish decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Publication {
    /// Nothing changed since the previous lock, which stands (RQ-117).
    Unchanged(Lock),
    /// A new lock, the labels it allocated, the citations of withdrawn labels it rewrote, and
    /// the files under `spec/` to write: `spec/labels.yaml` where it differs from the tree's,
    /// and the files whose citations it rewrote, the only files a publish changes (AC-460).
    Published {
        lock: Lock,
        files: Files,
        allocated: Vec<Allocated>,
        rewritten: Vec<Rewritten>,
    },
}

/// Why a publish refused before it wrote anything (F-27).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
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

/// Publishes a tree that reads: refuses on an uncommitted change under `spec/`, a branch other
/// than the publish branch or a validation error, in that order; otherwise allocates labels,
/// and reports the previous lock when the state root equals it, or gives the new lock and
/// bindings to write.
///
/// # Errors
///
/// A refusal, which writes nothing.
pub fn publish(
    tree: ReadTree<'_>,
    repo: &RepoState,
    previous: Option<&Lock>,
    at: PublishedAt,
) -> Result<Publication, Refusal> {
    let ReadTree {
        files,
        mut document,
        sites,
    } = tree;
    if !repo.modified.is_empty() {
        return Err(Refusal::Modified(repo.modified.clone()));
    }
    let branch = document.publish_branch.as_deref().unwrap_or("main");
    if repo.branch.as_deref() != Some(branch) {
        return Err(Refusal::Branch {
            current: repo.branch.clone(),
            publish: branch.to_owned(),
        });
    }
    let invalid = placed(validate(&document), &sites);
    if !invalid.is_empty() {
        return Err(Refusal::Invalid(invalid));
    }
    // Allocation gives only fresh labels of each element's own prefix, so the document stays
    // valid; when it gives none and rewrites nothing, the roots are the tree's own (AC-640).
    let allocated = allocate(&mut document);
    let (written, rewritten) = written(files, &document);
    if !rewritten.is_empty() {
        let mut next = files.clone();
        next.extend(written.clone());
        document = codec::read(&next).expect("rewritten citations read back");
    }
    let roots = hash_document(&document)
        .expect("a document the validator accepts indexes")
        .roots;
    if let Some(previous) = previous.filter(|lock| lock.state_root == roots.state) {
        return Ok(Publication::Unchanged(previous.clone()));
    }
    Ok(Publication::Published {
        lock: Lock {
            document_key: document.id,
            state_root: roots.state,
            content_root: roots.content,
            binding_root: roots.bindings,
            publish_sequence: previous.map_or(NonZeroU64::MIN, |lock| {
                lock.publish_sequence.saturating_add(1)
            }),
            published_at: at,
            derived_from: document.derived_from,
        },
        files: written,
        allocated,
        rewritten,
    })
}

/// The files under `spec/` a publish writes for `document`, the tree after allocation: the
/// bindings where they changed and the files whose citations of withdrawn labels it rewrites.
fn written(files: &Files, document: &Document) -> (Files, Vec<Rewritten>) {
    let rewrites = rewrite::rewrite(files, &document.bindings);
    let mut written = rewrites.files;
    let labels = codec::write_labels(document);
    if files.get(&SpecPath::new(LABELS)) != Some(&labels) {
        written.insert(SpecPath::new(LABELS), labels);
    }
    (written, rewrites.rewritten)
}

/// A root a lock records, in the order verification compares them (AC-678).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Root {
    Content,
    Binding,
    State,
}

impl Root {
    /// The root's name: `content`, `binding` or `state`; the lock field is the name followed by
    /// `Root`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::Binding => "binding",
            Self::State => "state",
        }
    }
}

impl fmt::Display for Root {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} root", self.name())
    }
}

/// Why a tree that reads does not verify against a lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unverified {
    /// The tree holds a merge conflict, or its references do not resolve, so it has no roots.
    Invalid(Vec<Placed>),
    /// The first root that differs (AC-469), with the lock's value and the tree's.
    Mismatch {
        root: Root,
        lock: Digest,
        tree: Digest,
    },
}

/// Recomputes the roots of a tree that reads and compares them with the lock's: the content
/// root, then the binding root, then the state root (RQ-118, AC-678). A tree that does not read
/// never gets this far, and a tree that holds a merge conflict compares no root, so no root is
/// compared for either (RQ-149). It writes nothing (AC-470).
///
/// # Errors
///
/// The tree has no roots, or a root differs.
pub fn verify(tree: &ReadTree<'_>, lock: &Lock) -> Result<Digest, Unverified> {
    let conflicts = conflict_errors(&tree.document);
    if !conflicts.is_empty() {
        return Err(Unverified::Invalid(placed(conflicts, &tree.sites)));
    }
    let roots = hash_document(&tree.document)
        .map_err(|errors| Unverified::Invalid(placed(index_errors(errors), &tree.sites)))?
        .roots;
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
