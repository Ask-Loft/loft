//! The entry points a caller other than `loft` relies on (Library entry points), each named from
//! outside the crate by its public path, with the types it takes: versions, trees, bases and
//! documents held in memory, a repository state only as an option, and no path.

use std::num::NonZeroUsize;

use crate::common::{key, product};
use loftspec::check::{Against, lint_against};
use loftspec::codec::Sites;
use loftspec::diff::{self, Comparison};
use loftspec::document::Document;
use loftspec::edit::{self, Found, InsertRefused, Parent, Subtree};
use loftspec::identity::Key;
use loftspec::ids::{DocumentId, FeatureId, NodeId};
use loftspec::lint::LintConfig;
use loftspec::publish::{self, Lock, Made, Publication, PublishedAt, ReadTree, RepoState};
use loftspec::review::{self, Page};

type Compare = fn(diff::Version<'_>, diff::Version<'_>) -> Result<Comparison, Vec<diff::Refusal>>;
type PageOf = fn(
    review::Version<'_>,
    Option<review::Version<'_>>,
    Option<NodeId>,
    usize,
    NonZeroUsize,
) -> Result<Page, review::Refused>;
type LintAgainst = fn(&Document, &Sites, &Document, &Sites, &LintConfig) -> Against;
type Publish = fn(
    ReadTree<'_>,
    Option<&RepoState>,
    Option<&Lock>,
    PublishedAt,
    Made,
) -> Result<Publication, publish::Refusal>;

type Find = fn(&Document, Key) -> Option<Found<'_>>;
type Detach = fn(&mut Document, NodeId) -> Result<Subtree, edit::Refusal>;
type Insert = fn(&mut Document, Parent, usize, Subtree) -> Result<(), InsertRefused>;
type Move = fn(&mut Document, NodeId, Parent, usize) -> Result<(), edit::Refusal>;

#[test]
fn each_entry_point_is_public_and_takes_only_what_is_held_in_memory() {
    let compare: Compare = diff::compare;
    let page: PageOf = review::page;
    let lint: LintAgainst = lint_against;
    let _: Publish = publish::publish;
    let find: Find = edit::find;
    let detach: Detach = edit::detach;
    let insert: Insert = edit::insert;
    let move_node: Move = edit::move_node;
    let document = Document::empty(DocumentId::new(key(0)));
    let version = || diff::Version {
        document: &document,
        lint: None,
    };
    assert!(compare(version(), version()).is_ok());
    let none = Sites::default();
    let against = lint(&document, &none, &document, &none, &LintConfig::default());
    assert_eq!(against.findings, []);
    let tree = review::Version {
        document: &document,
        sites: &none,
    };
    assert!(page(tree, None, None, 1000, NonZeroUsize::MIN).is_ok());
    let unheld: NodeId = FeatureId::new(key(1)).into();
    assert_eq!(find(&document, key(1)), None);
    let mut edited = document.clone();
    assert_eq!(
        detach(&mut edited, unheld),
        Err(edit::Refusal::UnknownNode(unheld))
    );
    let refused = move_node(&mut edited, unheld, Parent::ROOT, 0);
    assert_eq!(refused, Err(edit::Refusal::UnknownNode(unheld)));
    let product = Subtree::new(product(key(2), "Shelf"));
    insert(&mut edited, Parent::ROOT, 0, product).unwrap();
    assert_eq!(edited.products.len(), 1);
}

type Validated = fn(&Document, &Sites) -> Vec<loftspec::check::Placed>;
type Linted = fn(
    &Document,
    &Sites,
    Option<&[u8]>,
) -> Result<Vec<loftspec::lint::Finding>, Vec<loftspec::codec::ReadError>>;
type OfDocument = fn(
    &Document,
    Option<&'static str>,
) -> Result<ReadTree<'static>, Vec<loftspec::codec::ReadError>>;

#[test]
fn each_entry_point_on_a_document_held_in_memory_is_public() {
    let validated: Validated = loftspec::check::validated;
    let linted: Linted = loftspec::check::linted;
    let of_document: OfDocument = ReadTree::of_document;
    let document = crate::common::small_document();
    let sites = loftspec::codec::sites(&document);
    assert_eq!(validated(&document, &sites), []);
    assert!(linted(&document, &sites, None).is_ok());
    let tree = of_document(&document, None).unwrap();
    assert_eq!(tree.sites(), &sites);
}

type Settle = fn(
    &Document,
    loftspec::conflict::ConflictId,
    &loftspec::conflict::Settlement,
) -> Result<Document, loftspec::conflict::SettleError>;
type Owner = fn(&loftspec::codec::Rec) -> Option<loftspec::conflict::ConflictOwner>;

#[test]
fn settling_a_conflict_and_the_owner_of_a_records_conflicts_are_public() {
    let settle: Settle = loftspec::conflict::settle;
    let owner: Owner = loftspec::codec::conflict_owner;
    let document = crate::common::small_document();
    let unknown = loftspec::conflict::Conflict {
        owner: loftspec::conflict::ConflictOwner::Manifest,
        field: loftspec::conflict::FieldPath::new("publishBranch"),
        base: None,
        ours: None,
        theirs: None,
    }
    .id();
    let refused = settle(&document, unknown, &loftspec::conflict::Settlement::Ours);
    assert_eq!(
        refused,
        Err(loftspec::conflict::SettleError::Unknown(unknown))
    );
    let manifest = loftspec::codec::Rec::new(loftspec::codec::schema::Kind::Manifest);
    assert_eq!(
        owner(&manifest),
        Some(loftspec::conflict::ConflictOwner::Manifest)
    );
}
