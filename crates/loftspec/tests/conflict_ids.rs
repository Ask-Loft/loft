//! Conflict records as the validator lists them (F-34): each named by its identifier, its
//! holder and its field, the records of a tree in document order.

use crate::common::{
    component, feature, key, product, proposed_decision, root, small_document, term,
};
use loftspec::codec;
use loftspec::conflict::{Conflict, ConflictOwner, ConflictValue, FieldPath};
use loftspec::document::Document;
use loftspec::ids::{
    CategoryId, ComponentId, DecisionId, FeatureId, ProductId, RequirementId, TagId, TermId,
};
use loftspec::validate::{ValidationError, validate};

fn conflict(owner: ConflictOwner, field: &str) -> Conflict {
    Conflict {
        owner,
        field: FieldPath::new(field),
        base: Some(ConflictValue::Scalar("1".to_owned())),
        ours: Some(ConflictValue::Scalar("2".to_owned())),
        theirs: Some(ConflictValue::Scalar("3".to_owned())),
    }
}

/// The owner and field of each conflict the validator reports for `document` read back from
/// its tree, in the order it reports them.
fn listed(document: &Document) -> Vec<(ConflictOwner, String)> {
    let read = codec::read(&codec::write(document)).unwrap();
    validate(&read)
        .into_iter()
        .filter_map(|error| match error {
            ValidationError::Conflict { owner, field, .. } => Some((owner, field.to_string())),
            _ => None,
        })
        .collect()
}

/// The small document with a second Product listed after the first but filed before it, a
/// Feature filed after `Keys` but listed before it, a root Component and a decision filed before
/// the first ones, and a term.
fn spread() -> Document {
    let mut document = small_document();
    document.products.push(product(key(60), "Another Product"));
    let group = &mut document.products[0].feature_groups[0];
    group
        .features
        .insert(0, feature(key(40), "Labels", Vec::new()));
    document
        .components
        .push(root(component(key(61), "Archive")));
    document
        .decisions
        .push(proposed_decision(key(62), "a-proposal"));
    document.terms.push(term(63, "key", &[]));
    document
}

#[test]
fn the_conflict_records_of_a_tree_are_listed_in_document_order() {
    let mut document = spread();
    let element = |id: loftspec::ids::ElementId| ConflictOwner::Element(id);
    let expected = vec![
        (ConflictOwner::Manifest, "publishBranch"),
        (element(ProductId::new(key(1)).into()), "body"),
        (element(FeatureId::new(key(40)).into()), "body"),
        (element(FeatureId::new(key(3)).into()), "body"),
        (element(ProductId::new(key(60)).into()), "body"),
        (element(ComponentId::new(key(61)).into()), "body"),
        (element(ComponentId::new(key(6)).into()), "body"),
        (element(DecisionId::new(key(62)).into()), "title"),
        (element(DecisionId::new(key(20)).into()), "title"),
        (ConflictOwner::Tag(TagId::new(key(31))), "definition"),
        (
            ConflictOwner::Category(CategoryId::new(key(32))),
            "definition",
        ),
        (ConflictOwner::Term(TermId::new(key(63))), "definition"),
        (ConflictOwner::Labels, "counters.F"),
    ];
    for (owner, field) in expected.iter().rev() {
        document.conflicts.push(conflict(*owner, field));
    }
    let expected: Vec<(ConflictOwner, String)> = expected
        .into_iter()
        .map(|(owner, field)| (owner, field.to_owned()))
        .collect();
    assert_eq!(listed(&document), expected);
}

#[test]
fn within_one_file_conflict_records_are_listed_in_the_order_they_appear_in_it() {
    let mut document = small_document();
    let feature = ConflictOwner::Element(FeatureId::new(key(3)).into());
    let requirement = ConflictOwner::Element(RequirementId::new(key(4)).into());
    document.conflicts = vec![
        conflict(feature, "name"),
        conflict(feature, "body"),
        conflict(requirement, "body"),
    ];
    let expected = [(requirement, "body"), (feature, "name"), (feature, "body")]
        .map(|(owner, field)| (owner, field.to_owned()));
    assert_eq!(listed(&document), expected);
}
