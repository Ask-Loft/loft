//! The comparer's parts and refusals (Comparing two versions): whether the registries, the
//! term list, the lint configuration and the bindings differ, and the versions it refuses to
//! compare.

use crate::common::comparing::{compared, version};
use crate::common::{key, label, small_document, term};
use loftspec::codec::SpecPath;
use loftspec::conflict::{Conflict, ConflictOwner, FieldPath};
use loftspec::diff::{Part, Refusal, Version, Which, compare};
use loftspec::document::Document;
use loftspec::ids::RequirementId;

/// The flags of comparing the small document with itself edited by `edit`.
fn flags(edit: impl Fn(&mut Document)) -> [bool; 5] {
    let mut newer = small_document();
    edit(&mut newer);
    let c = compared(&small_document(), &newer);
    [
        Part::Tags,
        Part::Categories,
        Part::Terms,
        Part::LintConfiguration,
        Part::Bindings,
    ]
    .map(|part| c.differs(part))
}

#[test]
fn tags_and_categories_are_true_exactly_when_their_registries_differ() {
    let tag = flags(|d| d.tags[0].definition.push_str(" More."));
    assert_eq!(tag, [true, false, false, false, false]);
    let category = flags(|d| d.categories[0].definition.push_str(" More."));
    assert_eq!(category, [false, true, false, false, false]);
}

#[test]
fn terms_are_true_exactly_when_the_term_lists_differ_none_being_an_empty_list() {
    assert_eq!(flags(|_| {}), [false; 5]);
    let added = flags(|d| d.terms.push(term(50, "node", &[])));
    assert_eq!(added, [false, false, true, false, false]);
}

#[test]
fn the_lint_configuration_is_true_exactly_when_the_texts_differ_or_one_alone_holds_one() {
    let document = small_document();
    let with = |lint: Option<&'static str>| Version {
        document: &document,
        lint,
    };
    let differ = |a, b| {
        let comparison = compare(with(a), with(b)).unwrap();
        comparison.differs(Part::LintConfiguration)
    };
    assert!(!differ(None, None));
    assert!(!differ(Some("rules:\n"), Some("rules:\n")));
    assert!(differ(Some("rules:\n"), Some("rules: []\n")));
    assert!(differ(None, Some("rules:\n")));
    assert!(differ(Some("rules:\n"), None));
}

#[test]
fn bindings_are_true_exactly_when_a_counter_or_a_binding_field_differs() {
    let alias = flags(|d| {
        d.bindings
            .add_alias(RequirementId::new(key(4)), label("RQ-7"))
            .unwrap();
    });
    assert_eq!(alias, [false, false, false, false, true]);
    let counter = flags(|d| d.counters.raise(label("RQ-9")));
    assert_eq!(counter, [false, false, false, false, true]);
}

#[test]
fn a_version_with_a_node_without_a_key_is_refused_naming_the_version_and_the_file() {
    let mut newer = small_document();
    newer.products[0].feature_groups[0].features[0].requirements[0].criteria[0].id = None;
    let refused = compare(version(&small_document()), version(&newer)).unwrap_err();
    let feature = "products/example-product/identity/keys.yaml";
    assert_eq!(
        refused,
        [Refusal::Unkeyed {
            version: Which::Newer,
            path: SpecPath::new(feature),
        }]
    );
    let back = compare(version(&newer), version(&small_document())).unwrap_err();
    assert!(matches!(
        back[0],
        Refusal::Unkeyed {
            version: Which::Older,
            ..
        }
    ));
}

#[test]
fn a_version_with_a_conflict_record_is_refused_naming_the_version_and_the_record_s_key() {
    let mut older = small_document();
    let owner = ConflictOwner::Element(RequirementId::new(key(4)).into());
    older.conflicts.push(Conflict {
        owner,
        field: FieldPath::new("body"),
        base: None,
        ours: None,
        theirs: None,
    });
    let refused = compare(version(&older), version(&small_document())).unwrap_err();
    assert_eq!(
        refused,
        [Refusal::Conflict {
            version: Which::Older,
            owner,
        }]
    );
    assert_eq!(owner.key(), Some(key(4)));
}
