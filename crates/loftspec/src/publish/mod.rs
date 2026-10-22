//! The publisher (CMP-9): the preconditions of a publish, label allocation, the lock and its
//! verification. It decides from the tree, the repository's state and the previous lock it is
//! given; reading the repository and writing files belong to a store.

mod labels;
mod lock;
mod repo;
pub mod rewrite;
pub mod settle;

use std::borrow::Cow;
use std::fmt;
use std::num::NonZeroU64;

pub use labels::{
    Allocated, Allocation, NamedAsFormer, allocate, publish_labels, record_decisions,
};
pub use lock::{LOCK, Lock, LockError};
pub use loft_common::time::{Time as PublishedAt, TimeError};
pub use repo::{Operation, RepoState, Stopped};

use labels::{elements_in_order, named_as_former, publish_labels_in};
use settle::Settlement;

use crate::canonical::Digest;
use crate::check::{Placed, hashed, placed};
use crate::codec::{
    self, Files, LABELS, LINT, ReadError, Sites, SpecPath, lint_configuration, read_sited,
};
use crate::document::Document;
use crate::hash::{Roots, StateView, hash_document, state_root};
use crate::identity::{Key, Label};
use crate::validate::validate;

/// A label that a publish replaced in an element: a citation it rewrote in the element's text,
/// or a label it withdrew from the element's binding; `withdrawn` gave way to `label`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relabel {
    pub element: Key,
    pub withdrawn: Label,
    pub label: Label,
}

/// A tree read for a publish or a verification: its files, its document and where each of its
/// elements sits. Reading it first is what makes a read error come before every other answer.
pub struct ReadTree<'a> {
    files: Cow<'a, Files>,
    document: Document,
    sites: Sites,
    /// The text of `spec/lint.yaml`; none for a tree without the file.
    lint: Option<&'a str>,
}

impl<'a> ReadTree<'a> {
    /// Reads a `spec/` tree.
    ///
    /// # Errors
    ///
    /// Every read error the tree holds.
    pub fn read(files: &'a Files) -> Result<Self, Vec<ReadError>> {
        let (document, sites) = read_sited(files)?;
        let lint = lint_configuration(files).map_err(|error| vec![error])?;
        Ok(Self {
            files: Cow::Borrowed(files),
            document,
            sites,
            lint,
        })
    }

    /// The tree of the files the writer writes for a document held in memory, `spec/lint.yaml`
    /// holding `lint` where it is given: the tree [`ReadTree::read`] reads from those files, its
    /// document in the order the reader reads it. A document whose records hold only what the
    /// strict reader accepts is read without parsing its files.
    ///
    /// # Errors
    ///
    /// Every read error the written files hold, as [`ReadTree::read`] gives them.
    pub fn of_document(document: &Document, lint: Option<&'a str>) -> Result<Self, Vec<ReadError>> {
        let (mut files, read) = codec::write_and_read(document);
        let (document, sites) = read?;
        if let Some(text) = lint {
            files.insert(SpecPath::new(LINT), text.as_bytes().to_vec());
        }
        Ok(Self {
            files: Cow::Owned(files),
            document,
            sites,
            lint,
        })
    }

    /// The tree's document.
    #[must_use]
    pub const fn document(&self) -> &Document {
        &self.document
    }

    /// Where each of the tree's elements sits.
    #[must_use]
    pub const fn sites(&self) -> &Sites {
        &self.sites
    }

    /// The tree's roots and its state root under the highest state view, the one a publish
    /// records.
    ///
    /// # Errors
    ///
    /// The tree holds a merge conflict, or its references do not resolve, so it has no roots.
    pub fn roots(&self) -> Result<(Roots, Digest), Vec<Placed>> {
        let roots = hashed(&self.document, &self.sites)?.roots;
        let state = state_root(&self.document, &roots, StateView::HIGHEST, self.lint);
        Ok((roots, state))
    }
}

/// What a publish decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Publication {
    /// Nothing changed since the previous lock, which stands (RQ-117).
    Unchanged(Lock),
    /// A new lock, the labels it gave back to keys that came back, the labels it allocated, the
    /// citations of withdrawn labels it rewrote, and the files under `spec/` to write:
    /// `spec/labels.yaml` where it differs from the tree's, and the files whose citations it
    /// rewrote, the only files a publish changes (AC-460).
    /// A publish run in a stopped merge or rebase also gives the labels it withdrew and what
    /// stopped, which the user finishes.
    Published {
        lock: Lock,
        files: Files,
        given_back: Vec<Allocated>,
        allocated: Vec<Allocated>,
        rewritten: Vec<Relabel>,
        withdrawn: Vec<Relabel>,
        finish: Option<Operation>,
        /// The state the publish is based on, the one the previous lock names.
        base: Base,
    },
    /// A connected document's publish not made as its remote: the validated tree, unchanged,
    /// its roots, its state root under the highest state view, and the state the publish is based on. Its
    /// labels and its lock come from the remote, so no label is given, no binding changes and no
    /// lock is made.
    Prepared {
        document: Box<Document>,
        roots: Roots,
        state_root: Digest,
        base: Base,
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
    /// In a stopped merge or rebase, paths other than the bindings and the lock are unmerged.
    Unmerged(Vec<String>),
    /// In a stopped merge or rebase, `spec/labels.yaml` is unmerged but holds no conflict record
    /// over its bindings: git could not run the merge driver, so the dispute is not in the tree.
    UnmergedBindings,
    /// In a stopped merge or rebase, the lock at a commit is not a lock record.
    Lock {
        revision: &'static str,
        message: String,
    },
    /// The document is connected and the publish is not made as its remote, in a stopped merge
    /// or rebase: its labels come from that remote, so no settling of two publishes applies.
    Connected,
    /// The publish would give back labels that decisions of the tree name under `formerLabel`,
    /// which formatting writes as the node's key.
    NamedAsFormer(Vec<NamedAsFormer>),
}

/// Whether a publish is made as the document's remote, the one publisher of a connected
/// document that allocates its labels. A document that is not connected is published alike
/// either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Made {
    AsRemote,
    NotAsRemote,
}

/// The state a publish is based on: the publish sequence and the state root of the lock it is
/// made after, or sequence 0 and no state root without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Base {
    pub publish_sequence: u64,
    pub state_root: Option<Digest>,
}

impl Base {
    /// The base of a publish made after `previous`.
    #[must_use]
    pub fn of(previous: Option<&Lock>) -> Self {
        previous.map_or(
            Self {
                publish_sequence: 0,
                state_root: None,
            },
            |lock| Self {
                publish_sequence: lock.publish_sequence.get(),
                state_root: Some(lock.state_root),
            },
        )
    }
}

/// Publishes a tree that reads, after the lock `previous`, given the state of the repository
/// that holds it where there is one. With a repository state, outside a stopped merge or rebase
/// it refuses on an uncommitted change under `spec/` or, for a publish whose labels it
/// allocates, a branch other than the publish branch; in one it refuses as
/// [`settle::preconditions`] says and settles the labels two publishes gave apart. Then it
/// refuses on a validation error, and on a label it would give back that a decision names under
/// `formerLabel`; otherwise gives back departed labels, allocates labels, marks
/// departed the bindings of keys that left, rewrites citations of withdrawn labels, records each
/// decision's state, and reports the previous lock when the state root equals it, or gives the
/// new lock and the files to write. A connected document's publish not made as its remote is
/// refused in a stopped merge or rebase, and otherwise stops short of a lock, as
/// [`Publication::Prepared`].
///
/// # Errors
///
/// A refusal, which writes nothing.
pub fn publish(
    tree: ReadTree<'_>,
    repo: Option<&RepoState>,
    previous: Option<&Lock>,
    at: PublishedAt,
    made: Made,
) -> Result<Publication, Refusal> {
    let ReadTree {
        files,
        mut document,
        sites,
        lint,
    } = tree;
    let from_remote = document.connected && made == Made::NotAsRemote;
    let Begun {
        sequence,
        settlement,
        unrecorded,
    } = begin(&mut document, repo, previous, from_remote)?;
    let invalid = placed(validate(&document), &sites);
    if !invalid.is_empty() {
        return Err(Refusal::Invalid(invalid));
    }
    if from_remote {
        return Ok(prepared(document, lint, previous));
    }
    let elements = elements_in_order(&document);
    let named = named_as_former(&document, &elements);
    if !named.is_empty() {
        return Err(Refusal::NamedAsFormer(named));
    }
    // Allocation gives only fresh labels of each element's own prefix, so the document stays
    // valid; when it gives none and rewrites nothing, the roots are the tree's own (AC-640).
    let Allocation {
        given_back,
        allocated,
        departed,
    } = publish_labels_in(&mut document, &elements);
    let withdrawn = settlement.reattach(&mut document.bindings);
    let files = &*files;
    let rewrites = rewrite::rewrite(files, &document.bindings);
    let mut written = rewrites.files;
    if !written.is_empty() {
        // The rewritten text changes the roots, so the tree is read back as it will be written.
        document = read_back(files, &written, document);
    }
    // The records of the decisions are of the text the publish pins, and enter no root.
    record_decisions(&mut document);
    // Every lock is computed under the highest state view, which it names only where the tree
    // holds `spec/lint.yaml`: without the file every view gives the same roots.
    let (roots, state) = rooted(&document, lint);
    // A departed binding enters no root, so a tree whose bindings the publish gives back or
    // marks departed has changed since the lock even where the roots after the publish equal it.
    let settled = given_back.is_empty() && departed.is_empty();
    if let Some(previous) = previous.filter(|lock| settled && lock.state_root == state) {
        return Ok(Publication::Unchanged(previous.clone()));
    }
    if unrecorded {
        return Err(Refusal::UnmergedBindings);
    }
    write_labels(&mut written, files, &document);
    Ok(Publication::Published {
        lock: Lock {
            document_key: document.id,
            state_view: StateView::of(lint),
            state_root: state,
            content_root: roots.content,
            binding_root: roots.bindings,
            publish_sequence: sequence,
            published_at: at,
            derived_from: document.derived_from,
        },
        files: written,
        given_back,
        allocated,
        rewritten: rewrites.rewritten,
        withdrawn,
        finish: repo
            .and_then(|repo| repo.stopped.as_ref())
            .map(|stopped| stopped.operation),
        base: Base::of(previous),
    })
}

/// The publish of a connected document not made as its remote, stopped short of a lock: the
/// validated tree as it was read, its roots and its base, for the remote to publish.
fn prepared(document: Document, lint: Option<&str>, previous: Option<&Lock>) -> Publication {
    let (roots, state_root) = rooted(&document, lint);
    Publication::Prepared {
        state_root,
        roots,
        document: Box::new(document),
        base: Base::of(previous),
    }
}

/// The roots of a document the validator accepts, and its state root under the highest state
/// view, given the text of the tree's `spec/lint.yaml`.
fn rooted(document: &Document, lint: Option<&str>) -> (Roots, Digest) {
    let roots = hash_document(document)
        .expect("a document the validator accepts indexes")
        .roots;
    let state = state_root(document, &roots, StateView::HIGHEST, lint);
    (roots, state)
}

/// What the preconditions that read the repository give a publish: its sequence, what settling
/// two publishes did, and whether unmerged bindings lack a conflict record.
struct Begun {
    sequence: NonZeroU64,
    settlement: Settlement,
    unrecorded: bool,
}

/// Applies the preconditions that read the repository, where a repository state is given, and
/// in a stopped merge or rebase settles the labels two publishes gave apart. A publish whose
/// labels come from the remote is refused on no branch, and in a stopped merge or rebase.
fn begin(
    document: &mut Document,
    repo: Option<&RepoState>,
    previous: Option<&Lock>,
    from_remote: bool,
) -> Result<Begun, Refusal> {
    let next = previous.map_or(NonZeroU64::MIN, |lock| {
        lock.publish_sequence.saturating_add(1)
    });
    let unsettled = |sequence| Begun {
        sequence,
        settlement: Settlement::default(),
        unrecorded: false,
    };
    let Some(repo) = repo else {
        return Ok(unsettled(next));
    };
    let branch = document.publish_branch.as_deref().unwrap_or("main");
    if let Some(stopped) = &repo.stopped {
        if from_remote {
            return Err(Refusal::Connected);
        }
        let sequence = settle::preconditions(stopped, branch)?;
        let settlement = settle::settle(document, stopped.operation);
        // A tree the working lock already pins was settled; otherwise unmerged bindings without
        // a conflict record are refused once the roots are known.
        let unrecorded = settlement.unrecorded(stopped);
        return Ok(Begun {
            sequence,
            settlement,
            unrecorded,
        });
    }
    if !repo.modified.is_empty() {
        return Err(Refusal::Modified(repo.modified.clone()));
    }
    if !from_remote && repo.branch.as_deref() != Some(branch) {
        return Err(Refusal::Branch {
            current: repo.branch.clone(),
            publish: branch.to_owned(),
        });
    }
    Ok(unsettled(next))
}

/// The tree read back with the files a publish rewrote, as it will be written, keeping what the
/// publish settled in `settled`: its bindings, its counters and its conflicts, which the labels
/// it writes last will hold.
fn read_back(files: &Files, written: &Files, settled: Document) -> Document {
    let mut next = files.clone();
    next.extend(
        written
            .iter()
            .map(|(path, bytes)| (path.clone(), bytes.clone())),
    );
    let mut document = codec::read(&next).expect("rewritten citations read back");
    document.bindings = settled.bindings;
    document.counters = settled.counters;
    document.conflicts = settled.conflicts;
    document
}

/// Puts `spec/labels.yaml` among the files a publish writes where the bindings of `document`,
/// the tree after allocation, differ from the tree's.
fn write_labels(written: &mut Files, files: &Files, document: &Document) {
    let path = SpecPath::new(LABELS);
    let labels = codec::write_labels(document);
    if files.get(&path) != Some(&labels) {
        written.insert(path, labels);
    }
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
/// root, then the binding root, then the state root under the state view the lock names
/// (RQ-118, AC-678). A tree that does not read
/// never gets this far, and a tree that holds a merge conflict compares no root, so no root is
/// compared for either (RQ-149). It writes nothing (AC-470).
///
/// # Errors
///
/// The tree has no roots, or a root differs.
pub fn verify(tree: &ReadTree<'_>, lock: &Lock) -> Result<Digest, Unverified> {
    let roots = hashed(&tree.document, &tree.sites)
        .map_err(Unverified::Invalid)?
        .roots;
    let state = state_root(&tree.document, &roots, lock.state_view, tree.lint);
    match first_difference(lock, &roots, state) {
        Some((root, lock, tree)) => Err(Unverified::Mismatch { root, lock, tree }),
        None => Ok(state),
    }
}

/// The first root a lock records that differs from the tree's, in the order verification
/// compares them (AC-469, AC-678), with the lock's value and the tree's: `roots` are the tree's
/// roots and `state` its state root under the view in question.
#[must_use]
pub fn first_difference(
    lock: &Lock,
    roots: &Roots,
    state: Digest,
) -> Option<(Root, Digest, Digest)> {
    [
        (Root::Content, lock.content_root, roots.content),
        (Root::Binding, lock.binding_root, roots.bindings),
        (Root::State, lock.state_root, state),
    ]
    .into_iter()
    .find(|(_, lock, tree)| lock != tree)
}
