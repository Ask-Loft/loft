//! The review packer's pages (The review packet): the units of the packet of a tree and a base
//! held in memory, as JSON, filling each page whole up to a limit of characters, the same bytes
//! for every caller.

use std::num::NonZeroUsize;

use crate::common::{key, small_document};
use loftspec::codec::{self, Sites};
use loftspec::conflict::{Conflict, ConflictOwner, FieldPath};
use loftspec::document::Document;
use loftspec::ids::RequirementId;
use loftspec::review::{Page, Refused, Version, page};
use serde_json::Value;

fn version<'a>(document: &'a Document, sites: &'a Sites) -> Version<'a> {
    Version { document, sites }
}

fn number(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}

/// Page `n` of the small document's packet, with no base, under `limit`.
fn paged(limit: usize, n: usize) -> Result<Page, Refused> {
    let (document, sites) = (small_document(), Sites::default());
    page(version(&document, &sites), None, None, limit, number(n))
}

fn units(page: &Page) -> Vec<&Value> {
    page.units.iter().map(|unit| &unit.unit).collect()
}

/// The compact JSON size of each unit of the small document's packet on one page.
fn sizes() -> Vec<usize> {
    let whole = paged(usize::MAX, 1).unwrap();
    whole
        .units
        .iter()
        .map(|unit| serde_json::to_string(&unit.unit).unwrap().chars().count())
        .collect()
}

fn edited() -> Document {
    let mut document = small_document();
    let criterion =
        &mut document.products[0].feature_groups[0].features[0].requirements[0].criteria[0];
    "Every node record carries a key.".clone_into(&mut criterion.name);
    document
}

#[test]
fn the_tree_and_the_base_are_each_given_as_a_document_in_memory() {
    let (tree, base, none) = (edited(), small_document(), Sites::default());
    let page = page(
        version(&tree, &none),
        Some(version(&base, &none)),
        None,
        50_000,
        number(1),
    )
    .unwrap();
    assert_eq!(page.units.len(), 1);
    let assertions = page.units[0].unit["assertions"].as_array().unwrap();
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0]["change"], "changed");
    assert_eq!(assertions[0]["key"], key(5).to_string());
}

#[test]
fn a_caller_that_holds_no_repository_gets_a_packet_of_its_tree() {
    let page = paged(50_000, 1).unwrap();
    assert_eq!((page.units.len(), page.number, page.of), (3, 1, 1));
}

#[test]
fn two_callers_giving_the_same_versions_node_and_limit_get_pages_of_the_same_bytes() {
    let bytes = |page: &Page| serde_json::to_string(&units(page)).unwrap();
    let limit = sizes()[0] + 1;
    for n in 1..=3 {
        let (one, two) = (paged(limit, n).unwrap(), paged(limit, n).unwrap());
        assert_eq!(bytes(&one), bytes(&two), "page {n}");
        assert_eq!(one, two, "page {n}");
    }
}

#[test]
fn pages_take_every_next_whole_unit_that_fits_under_the_limit() {
    let sizes = sizes();
    assert_eq!(sizes.len(), 3);
    let limit = sizes[0] + sizes[1];
    let first = paged(limit, 1).unwrap();
    assert_eq!((first.units.len(), first.of), (2, 2));
    let taken: usize = first.units.iter().map(|unit| unit.chars).sum();
    assert_eq!(taken, limit);
    let second = paged(limit, 2).unwrap();
    assert_eq!(second.units.len(), 1);
    assert!(second.units[0].unit.get("oversized").is_none());
    assert_eq!(paged(limit - 1, 1).unwrap().units.len(), 1);
}

#[test]
fn a_unit_longer_than_the_limit_stands_alone_whole_and_marked_oversized() {
    let whole = paged(usize::MAX, 1).unwrap();
    let alone = paged(1, 2).unwrap();
    assert_eq!((alone.units.len(), alone.of), (1, 3));
    let mut expected = whole.units[1].unit.clone();
    expected["oversized"] = Value::Bool(true);
    assert_eq!(alone.units[0].unit, expected);
    assert_eq!(alone.units[0].chars, whole.units[1].chars);
}

#[test]
fn the_page_asked_for_comes_with_the_number_of_pages_and_one_past_the_last_is_refused() {
    let page = paged(1, 3).unwrap();
    assert_eq!((page.number, page.of), (3, 3));
    assert_eq!(paged(1, 4).unwrap_err(), Refused::PastLast { of: 3 });
}

#[test]
fn each_node_carries_its_key_label_kind_site_name_and_view() {
    let files = codec::write(&small_document());
    let (document, sites) = codec::read_sited(&files).unwrap();
    let page = page(version(&document, &sites), None, None, 50_000, number(1)).unwrap();
    let feature = &page.units[1].unit["scope"]["node"];
    assert_eq!(feature["key"], key(3).to_string());
    assert_eq!(feature["label"], "F-1");
    assert_eq!(feature["kind"], "feature");
    assert_eq!(
        feature["path"],
        "spec/products/example-product/identity/keys.yaml"
    );
    assert_eq!(
        (feature["line"].as_u64(), feature["column"].as_u64()),
        (Some(1), Some(6))
    );
    assert_eq!(feature["name"], "Keys");
    assert!(feature["view"].is_array());
}

#[test]
fn a_tree_that_holds_a_conflict_record_is_refused_naming_its_file_and_key() {
    let files = codec::write(&small_document());
    let (mut document, sites) = codec::read_sited(&files).unwrap();
    document.conflicts.push(Conflict {
        owner: ConflictOwner::Element(RequirementId::new(key(4)).into()),
        field: FieldPath::new("body"),
        base: None,
        ours: None,
        theirs: None,
    });
    let Err(Refused::Conflicted(errors)) =
        page(version(&document, &sites), None, None, 50_000, number(1))
    else {
        panic!("the conflict is refused");
    };
    assert_eq!(errors.len(), 1);
    let text = errors[0].to_string();
    assert!(
        text.starts_with("spec/products/example-product/identity/keys.yaml:"),
        "{text}"
    );
    assert!(text.contains(&key(4).to_string()), "{text}");
}

#[test]
fn a_tree_that_holds_a_node_without_a_key_is_refused() {
    let mut document = small_document();
    document.products[0].feature_groups[0].features[0].requirements[0].criteria[0].id = None;
    let sites = Sites::default();
    let refused = page(version(&document, &sites), None, None, 50_000, number(1));
    assert!(matches!(refused, Err(Refused::Unkeyed(unkeyed)) if unkeyed.len() == 1));
}

#[test]
fn the_review_packer_changes_nothing_in_the_versions_it_is_given() {
    let (tree, base, none) = (edited(), small_document(), Sites::default());
    let (tree_before, base_before) = (tree.clone(), base.clone());
    let tree_version = version(&tree, &none);
    let _ = page(
        tree_version,
        Some(version(&base, &none)),
        None,
        100,
        number(1),
    );
    assert_eq!((tree, base), (tree_before, base_before));
    assert_eq!(none, Sites::default());
}
