//! Conflict identifiers (F-34): each conflict record named by a digest of its holder, its field
//! and its sides, derived from the record alone and never written into a file; and the
//! validator's list of a tree's conflict records, in document order, each with its identifier,
//! its holder and its field.

use std::collections::BTreeSet;

use crate::common::merge_tree::{edited, features, merged, two_features};
use crate::common::text::{FEATURE, SHALL_CONFLICT, on_requirement};
use crate::common::{
    component, feature, key, product, proposed_decision, root, small_document, term,
};
use loftspec::check::{Checked, check};
use loftspec::codec::{self, Files, SpecPath};
use loftspec::conflict::{Conflict, ConflictOwner, ConflictValue, FieldPath};
use loftspec::document::Document;
use loftspec::ids::{
    CategoryId, ComponentId, DecisionId, FeatureId, ProductId, RequirementId, TagId, TermId,
};
use loftspec::merge::merge_file;
use loftspec::validate::{ValidationError, validate};
use serde_json::Value;

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

/// The small document's tree with the conflict over the Requirement's `statement.shall`.
fn conflicted() -> Files {
    on_requirement(SHALL_CONFLICT)
}

/// The conflicts of a tree, read.
fn conflicts_of(files: &Files) -> Vec<Conflict> {
    codec::read(files).unwrap().conflicts
}

/// The one conflict of a tree.
fn only(files: &Files) -> Conflict {
    let [conflict] = conflicts_of(files).try_into().unwrap();
    conflict
}

#[test]
fn the_holder_of_a_conflict_record_is_the_key_of_the_record_that_lists_it() {
    let conflict = only(&conflicted());
    assert_eq!(conflict.view()[0], key(4).to_string().as_str());
    assert_eq!(conflict.owner.holder(), key(4).to_string());
}

#[test]
fn a_conflict_record_of_the_manifest_has_the_holder_spec_loftspec_yaml() {
    let conflict = conflict(ConflictOwner::Manifest, "publishBranch");
    assert_eq!(conflict.view()[0], "spec/loftspec.yaml");
}

#[test]
fn a_conflict_record_of_the_bindings_has_the_holder_spec_labels_yaml() {
    let conflict = conflict(ConflictOwner::Labels, "counters.F");
    assert_eq!(conflict.view()[0], "spec/labels.yaml");
}

#[test]
fn a_side_on_which_the_field_is_absent_enters_the_view_as_null() {
    let mut absent = conflict(ConflictOwner::Manifest, "publishBranch");
    absent.base = None;
    assert_eq!(absent.view()[2], Value::Null);
    assert_eq!(absent.view()[3], "2");
    assert_ne!(
        absent.id(),
        conflict(ConflictOwner::Manifest, "publishBranch").id()
    );
}

#[test]
fn the_identifier_is_written_as_64_lowercase_hexadecimal_digits() {
    let text = only(&conflicted()).id().to_string();
    assert_eq!(text.len(), 64, "{text}");
    assert!(
        text.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{text}"
    );
}

#[test]
fn changing_the_holder_the_field_or_any_side_changes_the_identifier() {
    let original = only(&conflicted());
    let mut changed = Vec::new();
    let mut holder = original.clone();
    holder.owner = ConflictOwner::Element(RequirementId::new(key(41)).into());
    changed.push(holder);
    let mut field = original.clone();
    field.field = FieldPath::new("statement.system");
    changed.push(field);
    for side in 0..3 {
        let mut other = original.clone();
        let slot = match side {
            0 => &mut other.base,
            1 => &mut other.ours,
            _ => &mut other.theirs,
        };
        *slot = Some(ConflictValue::Scalar("write keys.".to_owned()));
        changed.push(other);
    }
    for other in changed {
        assert_ne!(other.id(), original.id(), "{other:?}");
    }
}

/// The small document with two conflicts on its Requirement, in the order given.
fn two_on_the_requirement(first: &str, second: &str) -> Vec<Conflict> {
    let requirement = ConflictOwner::Element(RequirementId::new(key(4)).into());
    let mut document = small_document();
    document.conflicts = vec![conflict(requirement, first), conflict(requirement, second)];
    conflicts_of(&codec::write(&document))
}

#[test]
fn moving_a_conflict_record_within_its_list_leaves_its_identifier_unchanged() {
    let ids =
        |conflicts: Vec<Conflict>| -> BTreeSet<_> { conflicts.iter().map(Conflict::id).collect() };
    let before = two_on_the_requirement("body", "statement.shall");
    let after = two_on_the_requirement("statement.shall", "body");
    assert_eq!(before[0].field, after[1].field);
    assert_eq!(ids(before), ids(after));
}

#[test]
fn adding_or_removing_another_conflict_record_leaves_the_identifier_unchanged() {
    let alone = only(&conflicted());
    let mut document = codec::read(&conflicted()).unwrap();
    document.conflicts.push(conflict(alone.owner, "body"));
    let both = conflicts_of(&codec::write(&document));
    assert_eq!(both.len(), 2);
    let shall = both.iter().find(|conflict| conflict.field == alone.field);
    assert_eq!(shall.map(Conflict::id), Some(alone.id()));
}

#[test]
fn two_merges_that_write_one_conflict_record_under_one_holder_give_it_one_identifier() {
    let base = two_features();
    let ours = edited(&base, "Ours.");
    let theirs = edited(&base, "Theirs.");
    let [base_tree, ours_tree, theirs_tree] = [&base, &ours, &theirs].map(codec::write);
    let by_driver = merge_file(
        &SpecPath::new(FEATURE),
        &base_tree[&SpecPath::new(FEATURE)],
        &ours_tree[&SpecPath::new(FEATURE)],
        &theirs_tree[&SpecPath::new(FEATURE)],
    )
    .unwrap();
    let mut driven = ours_tree.clone();
    driven.insert(SpecPath::new(FEATURE), by_driver.bytes);
    let by_tree = merged(&base, &ours, &theirs);
    let mut later = base.clone();
    features(&mut later)[1].details.body = Some("Labels name nodes.".to_owned());
    let by_later = merged(&later, &edited(&later, "Ours."), &edited(&later, "Theirs."));
    let id = only(&driven).id();
    assert_eq!(only(&by_tree.files).id(), id);
    assert_eq!(only(&by_later.files).id(), id);
}

#[test]
fn no_file_of_the_format_holds_a_conflict_identifier() {
    let files = conflicted();
    let id = only(&files).id().to_string();
    for (path, bytes) in &codec::write(&codec::read(&files).unwrap()) {
        assert!(!String::from_utf8_lossy(bytes).contains(&id), "{path}");
    }
}

#[test]
fn no_conflict_record_holds_its_identifier() {
    let files = codec::write(&codec::read(&conflicted()).unwrap());
    let text = String::from_utf8(files[&SpecPath::new(FEATURE)].clone()).unwrap();
    let record = &text[text.find("    conflicts:\n").unwrap()..];
    let fields: Vec<&str> = record
        .lines()
        .skip(1)
        .take_while(|line| line.starts_with("      "))
        .map(|line| line.trim_start().trim_start_matches("- "))
        .filter_map(|line| line.split(':').next())
        .collect();
    assert_eq!(fields, ["field", "base", "ours", "theirs"]);
}

/// The validation errors a tree gives, as they print.
fn printed(files: &Files) -> Vec<String> {
    match check(files) {
        Checked::Read { invalid, .. } => invalid.iter().map(ToString::to_string).collect(),
        Checked::Unread(errors) => panic!("the tree reads: {errors:?}"),
    }
}

#[test]
fn the_error_for_a_conflict_record_names_its_identifier() {
    let files = conflicted();
    let id = only(&files).id().to_string();
    let [error] = printed(&files).try_into().unwrap();
    assert!(error.contains(&id), "{error}");
}

#[test]
fn the_error_for_a_conflict_record_names_its_holder() {
    let [error] = printed(&conflicted()).try_into().unwrap();
    assert!(error.contains(&key(4).to_string()), "{error}");
    let mut document = small_document();
    document.conflicts = vec![conflict(ConflictOwner::Manifest, "publishBranch")];
    let [error] = printed(&codec::write(&document)).try_into().unwrap();
    assert!(error.starts_with("spec/loftspec.yaml: "), "{error}");
}

#[test]
fn the_error_for_a_conflict_record_names_its_field() {
    let [error] = printed(&conflicted()).try_into().unwrap();
    assert!(error.ends_with("over `statement.shall`"), "{error}");
}

fn scalar(text: &str) -> ConflictValue {
    ConflictValue::Scalar(text.to_owned())
}

#[test]
fn a_side_written_as_deleted_enters_the_view_as_null() {
    let mut deleted = conflict(ConflictOwner::Manifest, "record");
    deleted.theirs = Some(ConflictValue::Deleted);
    assert_eq!(deleted.view()[4], Value::Null);
}

#[test]
fn a_scalar_side_enters_the_view_as_its_text_a_number_or_a_flag_included() {
    let mut counter = conflict(ConflictOwner::Labels, "counters.F");
    counter.ours = Some(scalar("12"));
    counter.theirs = Some(scalar("true"));
    assert_eq!(counter.view()[3], "12");
    assert_eq!(counter.view()[4], "true");
}

#[test]
fn a_sequence_side_enters_the_view_as_an_array_in_written_order() {
    let mut tags = conflict(ConflictOwner::Manifest, "tags");
    tags.ours = Some(ConflictValue::List(vec![scalar("zeta"), scalar("alpha")]));
    assert_eq!(tags.view()[3], serde_json::json!(["zeta", "alpha"]));
}

#[test]
fn a_mapping_side_enters_the_view_as_an_object_of_its_written_fields() {
    let mut statement = conflict(ConflictOwner::Manifest, "statement");
    let fields = vec![
        ("form".to_owned(), scalar("ubiquitous")),
        ("system".to_owned(), scalar("The writer")),
    ];
    statement.ours = Some(ConflictValue::Record(fields));
    let expected = serde_json::json!({"form": "ubiquitous", "system": "The writer"});
    assert_eq!(statement.view()[3], expected);
}

#[test]
fn a_side_of_extension_data_enters_the_view_as_the_json_it_holds() {
    let mut ext = conflict(ConflictOwner::Manifest, "ext");
    let namespace = loftspec::names::Namespace::parse("openspec").unwrap();
    let value = loftspec::ext::ExtValue::Integer(-3);
    ext.ours = Some(ConflictValue::Ext([(namespace, value)].into()));
    assert_eq!(ext.view()[3], serde_json::json!({"openspec": -3}));
}
