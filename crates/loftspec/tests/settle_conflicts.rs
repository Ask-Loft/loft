//! Settling one conflict of a document held in memory (Settling a conflict): by a side or by a
//! value, giving a document that reads without the conflict record, and the owner of the
//! conflicts a record holds.

use loftspec::codec;
use loftspec::conflict::{
    Conflict, ConflictId, ConflictOwner, ConflictValue, FieldPath, SettleError, Settlement, settle,
};
use loftspec::document::Document;
use loftspec::ids::{ElementId, RequirementId};
use loftspec::statement::{EarsForm, Statement};

use crate::common::merge_tree::{edited, requirement_4};
use crate::common::settle::{SECOND, THIRD, merge, over, refusal, settled, three_requirements};
use crate::common::{key, label, small_document};

fn bodied(document: &Document, body: &str) -> Document {
    edited(document, body)
}

/// A conflict over the body of the Requirement keyed 4, which base lacks.
fn body_conflict() -> Document {
    let base = small_document();
    merge(&base, &bodied(&base, "Ours."), &bodied(&base, "Theirs."))
}

fn body_of_4(document: &Document) -> Option<String> {
    requirement_4(&mut document.clone()).details.body.clone()
}

fn shall(document: &Document, text: &str) -> Document {
    let mut document = document.clone();
    requirement_4(&mut document).statement = Statement::Form(EarsForm::Ubiquitous {
        system: "The writer".to_owned(),
        shall: text.to_owned(),
    });
    document
}

/// Conflicts over the body and over `statement.shall` of the Requirement keyed 4.
fn two_conflicts() -> Document {
    let base = shall(&small_document(), "write a key.");
    let ours = bodied(&shall(&base, "write a slug."), "Ours.");
    let theirs = bodied(&shall(&base, "write a label."), "Theirs.");
    merge(&base, &ours, &theirs)
}

#[test]
fn settling_a_conflict_gives_a_document_without_its_conflict_record() {
    let document = body_conflict();
    let id = over(&document, "body").id();
    let settled = settle(&document, id, &Settlement::Ours).unwrap();
    assert!(settled.conflicts.iter().all(|each| each.id() != id));
    assert_eq!(settled.conflicts, []);
}

#[test]
fn settling_a_conflict_keeps_every_other_conflict_record() {
    let document = two_conflicts();
    let other = over(&document, "statement.shall").clone();
    let settled = settled(&document, "body", &Settlement::Theirs);
    assert_eq!(settled.conflicts, [other]);
}

#[test]
fn settling_a_conflict_leaves_the_document_given_unchanged() {
    let document = two_conflicts();
    let before = document.clone();
    let _ = settled(&document, "body", &Settlement::Ours);
    assert_eq!(document, before);
}

#[test]
fn settling_a_field_by_a_side_gives_it_that_sides_value() {
    let document = body_conflict();
    let ours = settled(&document, "body", &Settlement::Ours);
    let theirs = settled(&document, "body", &Settlement::Theirs);
    assert_eq!(body_of_4(&ours).as_deref(), Some("Ours."));
    assert_eq!(body_of_4(&theirs).as_deref(), Some("Theirs."));
}

#[test]
fn settling_a_field_by_a_side_that_lacks_it_leaves_the_field_out() {
    let document = body_conflict();
    assert_eq!(over(&document, "body").base, None);
    assert_eq!(
        body_of_4(&settled(&document, "body", &Settlement::Base)),
        None
    );
}

#[test]
fn settling_a_dotted_field_sets_it_in_the_record_the_path_leads_through() {
    let document = two_conflicts();
    let settled = settled(&document, "statement.shall", &Settlement::Base);
    let statement = requirement_4(&mut settled.clone()).statement.clone();
    let Statement::Form(EarsForm::Ubiquitous { shall, .. }) = statement else {
        panic!("{statement:?}");
    };
    assert_eq!(shall, "write a key.");
}

#[test]
fn settling_a_field_by_a_value_gives_it_that_value() {
    let document = body_conflict();
    let value = ConflictValue::Scalar("Settled.".to_owned());
    let settled = settled(&document, "body", &Settlement::Value(value));
    assert_eq!(body_of_4(&settled).as_deref(), Some("Settled."));
}

#[test]
fn a_conflict_of_the_manifest_is_settled_in_the_manifest() {
    let base = small_document();
    let mut ours = base.clone();
    ours.publish_branch = Some("one".to_owned());
    let mut theirs = base.clone();
    theirs.publish_branch = Some("two".to_owned());
    let document = merge(&base, &ours, &theirs);
    assert_eq!(
        over(&document, "publishBranch").owner,
        ConflictOwner::Manifest
    );
    let settled = settled(&document, "publishBranch", &Settlement::Theirs);
    assert_eq!(settled.publish_branch.as_deref(), Some("two"));
}

/// A conflict over bindings: ours binds the Requirement keyed 60 to `RQ-3`, theirs the one keyed
/// 61.
fn bindings_conflict() -> Document {
    let base = three_requirements();
    let mut ours = base.clone();
    let second = RequirementId::new(key(SECOND));
    ours.bindings.bind(second, label("RQ-3")).unwrap();
    let mut theirs = base.clone();
    let third = RequirementId::new(key(THIRD));
    theirs.bindings.bind(third, label("RQ-3")).unwrap();
    merge(&base, &ours, &theirs)
}

#[test]
fn a_conflict_of_the_labels_file_is_settled_in_its_record() {
    let document = bindings_conflict();
    assert_eq!(over(&document, "bindings").owner, ConflictOwner::Labels);
    let settled = settled(&document, "bindings", &Settlement::Theirs);
    let third = ElementId::Node(RequirementId::new(key(THIRD)).into());
    assert_eq!(settled.bindings.label_of(third), Some(label("RQ-3")));
}

#[test]
fn a_conflict_over_bindings_adds_the_sides_bindings_to_those_the_file_holds() {
    let document = bindings_conflict();
    let settled = settled(&document, "bindings", &Settlement::Ours);
    let second = ElementId::Node(RequirementId::new(key(SECOND)).into());
    let third = ElementId::Node(RequirementId::new(key(THIRD)).into());
    assert_eq!(settled.bindings.label_of(second), Some(label("RQ-3")));
    assert_eq!(settled.bindings.label_of(third), None);
    for binding in document.bindings.iter() {
        assert!(
            settled.bindings.iter().any(|each| each == binding),
            "{binding:?}"
        );
    }
    assert_eq!(
        settled.bindings.iter().count(),
        document.bindings.iter().count() + 1
    );
}

/// A conflict over the subject of the consequence keyed 25: ours names 60, theirs 61.
fn subject_conflict() -> Document {
    let base = three_requirements();
    let subject = |n: u32| {
        let mut document = base.clone();
        let consequence = &mut document.decisions[0].consequences[0];
        consequence.subject = Some(RequirementId::new(key(n)).into());
        document
    };
    merge(&base, &subject(SECOND), &subject(THIRD))
}

#[test]
fn a_value_in_the_shape_of_the_conflicts_sides_is_accepted() {
    let document = subject_conflict();
    let ours = over(&document, "subject").ours.clone().unwrap();
    let settled = settled(&document, "subject", &Settlement::Value(ours));
    let subject = settled.decisions[0].consequences[0].subject;
    assert_eq!(subject, Some(RequirementId::new(key(SECOND)).into()));
}

fn shape_refused(document: &Document, field: &str, value: ConflictValue) {
    assert_eq!(
        refusal(document, field, &Settlement::Value(value)),
        SettleError::Shape(FieldPath::new(field))
    );
}

fn text(text: &str) -> ConflictValue {
    ConflictValue::Scalar(text.to_owned())
}

#[test]
fn a_list_given_for_a_field_that_holds_one_value_is_refused_naming_the_field() {
    shape_refused(
        &body_conflict(),
        "body",
        ConflictValue::List(vec![text("A.")]),
    );
}

#[test]
fn a_scalar_given_for_a_field_that_holds_a_list_is_refused_naming_the_field() {
    let mut document = small_document();
    let tags = |name: &str| Some(ConflictValue::List(vec![text(name)]));
    document.conflicts.push(Conflict {
        owner: ConflictOwner::Element(ElementId::Node(RequirementId::new(key(4)).into())),
        field: FieldPath::new("tags"),
        base: None,
        ours: tags("portable"),
        theirs: tags("identity"),
    });
    let document = codec::read(&codec::write(&document)).expect("the conflict reads");
    shape_refused(&document, "tags", text("portable"));
}

#[test]
fn a_record_given_for_a_field_that_holds_a_scalar_is_refused_naming_the_field() {
    let record = ConflictValue::Record(vec![("label".to_owned(), text("RQ-1"))]);
    shape_refused(&body_conflict(), "body", record);
}

#[test]
fn a_record_given_with_a_field_its_kind_does_not_define_is_refused_naming_the_field() {
    let record = ConflictValue::Record(vec![("colour".to_owned(), text("blue"))]);
    shape_refused(&subject_conflict(), "subject", record);
}

#[test]
fn an_identifier_that_no_conflict_record_has_is_refused_naming_it() {
    let document = body_conflict();
    let mut other = over(&document, "body").clone();
    other.ours = Some(text("Elsewhere."));
    let unknown: ConflictId = other.id();
    assert_eq!(
        settle(&document, unknown, &Settlement::Ours),
        Err(SettleError::Unknown(unknown))
    );
}

#[test]
fn settling_a_slug_by_a_side_gives_the_record_that_slug_and_its_file_the_path_it_names() {
    let base = crate::common::merge_tree::two_features();
    let renamed = |text: &str| {
        let mut document = base.clone();
        crate::common::merge_tree::features(&mut document)[1].slug = crate::common::slug(text);
        document
    };
    let document = merge(&base, &renamed("labelling"), &renamed("label-rules"));
    let settled = settled(&document, "slug", &Settlement::Theirs);
    let slug = &settled.products[0].feature_groups[0].features[1].slug;
    assert_eq!(slug, &crate::common::slug("label-rules"));
    let path = codec::SpecPath::new("products/example-product/identity/label-rules.yaml");
    assert!(codec::write(&settled).contains_key(&path));
}
