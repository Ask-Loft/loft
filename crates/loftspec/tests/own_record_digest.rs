//! The own record digest (Own record views): the digest of a record's own record view under
//! `loftspec/own-record/1`, which every field the writer writes for the record moves and nothing
//! else does: not the records it holds, its place among its siblings, its parent or its key. No
//! root holds it.

use std::collections::BTreeSet;

use loftspec::canonical::{Digest, Domain, digest};
use loftspec::codec::{self, RecordKind, SpecPath, own_record_digest};
use loftspec::decision::{DecisionState, Rating, StateKind};
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::ext::ExtValue;
use loftspec::hash::hash_document;
use loftspec::ids::{ComponentId, CriterionId, DecisionId, OptionId, RequirementId};
use loftspec::names::{Namespace, TagName};
use loftspec::statement::{EarsForm, Statement};
use loftspec::tree::{Feature, Requirement};
use proptest::prelude::*;

use crate::common::edits::{Side, edit, edited, growth};
use crate::common::full::full_nested_document;
use crate::common::merge_cases::{FIXTURES, fixture_files};
use crate::common::own::{
    COMPONENT, CONSTRAINT, CRITERION, FEATURE, GROUP, PRODUCT, QUALITY, REQUIREMENT, digest_of, own,
};
use crate::common::{
    criterion, feature, fresh_key, key, label, small_document, small_document_with, term,
};

/// A change to a document, named for the field it changes in the record it is checked on.
type Change = (&'static str, RecordKind, u32, fn(&mut Document));

/// The full nested document with a term list.
fn base() -> Document {
    let mut document = full_nested_document();
    document.terms = vec![term(70, "record", &["row"])];
    document
}

fn the_feature(document: &mut Document) -> &mut Feature {
    &mut document.products[0].feature_groups[0].features[0]
}

fn the_requirement(document: &mut Document) -> &mut Requirement {
    &mut the_feature(document).requirements[0]
}

/// Asserts that each change moves the own record digest of the record it names.
fn each_changes(changes: &[Change]) {
    let document = base();
    for (field, kind, n, change) in changes {
        let mut edited = document.clone();
        change(&mut edited);
        assert_ne!(
            digest_of(&edited, *kind, *n),
            digest_of(&document, *kind, *n),
            "{field} of {kind:?} {n}"
        );
    }
}

/// Asserts that the change leaves the own record digest of every record of `kept` as it was.
fn keeps(change: impl FnOnce(&mut Document), kept: &[(RecordKind, u32)]) {
    let document = base();
    let mut edited = document.clone();
    change(&mut edited);
    for (kind, n) in kept {
        assert_eq!(
            digest_of(&edited, *kind, *n),
            digest_of(&document, *kind, *n),
            "{kind:?} {n}"
        );
    }
}

#[test]
fn the_own_record_digest_is_the_digest_of_its_view_under_its_domain() {
    assert_eq!(Domain::OwnRecord.as_str(), "loftspec/own-record/1");
    for record in own(&base()).values() {
        assert_eq!(record.digest, own_record_digest(&record.view));
        assert_eq!(record.digest, digest(Domain::OwnRecord, &record.view));
    }
}

#[test]
fn a_change_to_a_nodes_slug_name_body_or_statement_changes_its_digest() {
    each_changes(&[
        ("slug", PRODUCT, 1, |d| {
            d.products[0].slug = crate::common::slug("other");
        }),
        ("slug", GROUP, 2, |d| {
            d.products[0].feature_groups[0].slug = crate::common::slug("other");
        }),
        ("slug", FEATURE, 3, |d| {
            the_feature(d).slug = crate::common::slug("other");
        }),
        ("slug", COMPONENT, 6, |d| {
            d.components[0].slug = crate::common::slug("other");
        }),
        ("name", CRITERION, 5, |d| {
            the_requirement(d).criteria[0].name = "Other.".to_owned();
        }),
        ("body", FEATURE, 3, |d| {
            the_feature(d).details.body = Some("Other.".to_owned());
        }),
        ("body", FEATURE, 3, |d| the_feature(d).details.body = None),
        ("statement form", REQUIREMENT, 4, |d| {
            the_requirement(d).statement = Statement::Form(EarsForm::EventDriven {
                when: "a node is written".to_owned(),
                system: "The writer".to_owned(),
                shall: "write a key on every node.".to_owned(),
            });
        }),
        ("statement part", REQUIREMENT, 4, |d| {
            the_requirement(d).statement = Statement::Form(EarsForm::Ubiquitous {
                system: "The formatter".to_owned(),
                shall: "write a key on every node.".to_owned(),
            });
        }),
        ("statement text", REQUIREMENT, 4, |d| {
            the_requirement(d).statement = Statement::Text("The writer writes keys.".to_owned());
        }),
    ]);
}

#[test]
fn a_change_to_a_nodes_tags_or_ext_changes_its_digest() {
    let portable = || TagName::parse("portable").unwrap();
    each_changes(&[
        ("tag added", PRODUCT, 1, |d| {
            d.products[0]
                .details
                .tags
                .push(TagName::parse("portable").unwrap());
        }),
        ("tag repeated", PRODUCT, 1, |d| {
            let tags = &mut d.products[0].details.tags;
            tags.push(tags[0].clone());
        }),
        ("ext", FEATURE, 3, |d| {
            the_feature(d)
                .details
                .ext
                .insert(Namespace::parse("tool").unwrap(), ExtValue::Boolean(true));
        }),
    ]);
    let mut ordered = base();
    ordered.products[0].details.tags.push(portable());
    let mut reordered = ordered.clone();
    reordered.products[0].details.tags.reverse();
    assert_ne!(
        digest_of(&ordered, PRODUCT, 1),
        digest_of(&reordered, PRODUCT, 1)
    );
}

#[test]
fn a_change_to_an_edge_a_record_writes_changes_its_digest() {
    each_changes(&[
        ("realizes", FEATURE, 3, |d| {
            d.edges.retain(|e| !matches!(e, Edge::Realizes { .. }));
        }),
        ("scopedTo", COMPONENT, 6, |d| {
            d.edges.retain(|e| !matches!(e, Edge::ScopedTo { .. }));
        }),
        ("mentions", CRITERION, 5, |d| {
            d.edges.push(Edge::Mentions {
                source: CriterionId::new(key(5)).into(),
                target: ComponentId::new(key(6)).into(),
            });
        }),
        ("subjects", RecordKind::Decision, 20, |d| {
            for edge in &mut d.edges {
                if let Edge::Subject { role, .. } = edge {
                    *role = SubjectRole::Affected;
                }
            }
        }),
    ]);
}

#[test]
fn a_change_to_a_typed_attribute_changes_its_digest() {
    use loftspec::attributes::{Boundary, ConservationKind, QaKind, VerificationFate};
    each_changes(&[
        ("boundary", COMPONENT, 6, |d| {
            d.components[0].component.boundary = Some(Boundary::External);
        }),
        ("conservationKind", CONSTRAINT, 7, |d| {
            d.components[0].component.constraints[0].conservation_kind =
                Some(ConservationKind::SumPreservation);
        }),
        ("constraintBinding", CONSTRAINT, 7, |d| {
            let constraint = &mut d.components[0].component.constraints[0];
            constraint.binding.as_mut().unwrap().required_value = "go".to_owned();
        }),
        ("verificationFate", CONSTRAINT, 9, |d| {
            d.products[0].feature_groups[0].constraints[0].verification_fate =
                Some(VerificationFate::Sampled);
        }),
        ("qaKind", QUALITY, 8, |d| {
            d.components[0].component.quality_attributes[0].kind = QaKind::Throughput;
        }),
        ("qualityTarget", QUALITY, 8, |d| {
            d.components[0].component.quality_attributes[0].target.unit = "ms".to_owned();
        }),
    ]);
}

#[test]
fn a_change_to_a_decision_or_one_of_its_parts_changes_its_digest() {
    const ADR: RecordKind = RecordKind::Decision;
    each_changes(&[
        ("slug", ADR, 20, |d| {
            d.decisions[0].slug = crate::common::slug("other");
        }),
        ("title", ADR, 20, |d| {
            d.decisions[0].title = "Other".to_owned();
        }),
        ("context", ADR, 20, |d| {
            d.decisions[0].context = "Other.".to_owned();
        }),
        ("state", ADR, 20, |d| {
            d.decisions[0].state = DecisionState::Proposed;
        }),
        ("state outcome", ADR, 20, |d| {
            d.decisions[0].state = DecisionState::Accepted {
                selected_option: OptionId::new(key(23)),
                outcome: "Other.".to_owned(),
            };
        }),
        ("option", ADR, 20, |d| {
            d.decisions[0].options[1].description = "Other.".to_owned();
        }),
        ("option key", ADR, 20, |d| {
            d.decisions[0].options[1].id = OptionId::new(key(77));
        }),
        ("driver", ADR, 20, |d| d.decisions[0].drivers.truncate(1)),
        ("assessment", ADR, 20, |d| {
            d.decisions[0].assessments[0].rating = Rating::Supports;
        }),
        ("consequence", ADR, 20, |d| {
            d.decisions[0].consequences[1].statement = "Other.".to_owned();
        }),
    ]);
}

#[test]
fn a_change_to_a_registry_entry_or_a_binding_changes_its_digest() {
    const BINDING: RecordKind = RecordKind::Binding;
    each_changes(&[
        ("definition", RecordKind::Tag, 30, |d| {
            d.tags[1].definition = "Other.".to_owned();
        }),
        ("name", RecordKind::Category, 32, |d| {
            d.categories[0].name = loftspec::names::CategoryName::parse("runtime.os").unwrap();
        }),
        ("aliases", RecordKind::Term, 70, |d| {
            d.terms[0].aliases.push("line".to_owned());
        }),
        ("label", BINDING, 4, |d| {
            d.bindings
                .relabel(RequirementId::new(key(4)), label("RQ-3"))
                .unwrap();
        }),
        ("aliases", BINDING, 20, |d| {
            d.bindings
                .add_alias(DecisionId::new(key(20)), label("ADR-9"))
                .unwrap();
        }),
        ("withdrawn", BINDING, 20, |d| {
            d.bindings
                .add_withdrawn(DecisionId::new(key(20)), label("ADR-8"))
                .unwrap();
        }),
        ("departed", BINDING, 20, |d| {
            d.bindings.depart(DecisionId::new(key(20))).unwrap();
        }),
        ("publishedState", BINDING, 20, |d| {
            let id = DecisionId::new(key(20));
            d.bindings
                .record_published(id, Some(StateKind::Accepted), None)
                .unwrap();
        }),
    ]);
}

#[test]
fn adding_removing_or_editing_a_record_a_record_holds_leaves_the_holders_digest() {
    let held = [
        (PRODUCT, 1),
        (GROUP, 2),
        (FEATURE, 3),
        (REQUIREMENT, 4),
        (COMPONENT, 6),
    ];
    keeps(
        |d| {
            the_requirement(d)
                .criteria
                .push(criterion(key(71), "A second criterion."));
            the_requirement(d).criteria[0].name = "Edited.".to_owned();
            the_feature(d).details.sections.clear();
            let copy = the_requirement(d).clone();
            the_feature(d).requirements.push(copy);
            the_feature(d).requirements[1].id = Some(RequirementId::new(key(72)));
            the_feature(d).requirements[1].criteria.clear();
            the_feature(d).requirements[1].details.sections.clear();
            let group = &mut d.products[0].feature_groups[0];
            group.features.push(feature(key(73), "Labels", Vec::new()));
            group.constraints.clear();
            d.products[0].constraints.clear();
            d.components[0].component.components.clear();
        },
        &held,
    );
}

#[test]
fn moving_a_record_among_its_siblings_leaves_its_digest() {
    let mut document = base();
    the_requirement(&mut document)
        .criteria
        .push(criterion(key(71), "A second criterion."));
    let before = [
        digest_of(&document, CRITERION, 5),
        digest_of(&document, CRITERION, 71),
    ];
    the_requirement(&mut document).criteria.reverse();
    let after = [
        digest_of(&document, CRITERION, 5),
        digest_of(&document, CRITERION, 71),
    ];
    assert_eq!(after, before);
}

#[test]
fn moving_a_record_to_another_parent_leaves_its_digest() {
    keeps(
        |d| {
            let group = &mut d.products[0].feature_groups[0];
            let constraint = group.constraints.remove(0);
            d.products[0].constraints.push(constraint);
            let quality = the_feature(d).quality_attributes.remove(0);
            d.products[0].feature_groups[0]
                .quality_attributes
                .push(quality);
            let requirement = the_feature(d).requirements.remove(0);
            let group = &mut d.products[0].feature_groups[0];
            group
                .features
                .push(feature(key(73), "Labels", vec![requirement]));
        },
        &[
            (CONSTRAINT, 9),
            (QUALITY, 10),
            (REQUIREMENT, 4),
            (CRITERION, 5),
        ],
    );
}

#[test]
fn two_records_with_the_same_content_and_different_keys_have_the_same_digest() {
    let mut document = base();
    let criteria = &mut the_requirement(&mut document).criteria;
    let mut copy = criteria[0].clone();
    copy.id = Some(CriterionId::new(key(71)));
    copy.details.sections.clear();
    criteria.push(copy);
    document.edges.push(Edge::Mentions {
        source: CriterionId::new(key(71)).into(),
        target: RequirementId::new(key(4)).into(),
    });
    assert_eq!(
        digest_of(&document, CRITERION, 71),
        digest_of(&document, CRITERION, 5)
    );
}

#[test]
fn the_same_record_in_two_documents_has_the_same_digest() {
    let mut other = small_document();
    other.products[0].details.body = Some("Another example.".to_owned());
    other.products[0].feature_groups[0]
        .features
        .push(feature(key(73), "Labels", Vec::new()));
    let small = small_document();
    for (kind, n) in [(CRITERION, 5), (REQUIREMENT, 4), (RecordKind::Decision, 20)] {
        assert_eq!(
            digest_of(&other, kind, n),
            digest_of(&small, kind, n),
            "{n}"
        );
    }
    let fresh = small_document_with(&fresh_key);
    let keyless = |document: &Document, kind: RecordKind| -> Vec<Digest> {
        let records = own(document);
        records
            .into_iter()
            .filter(|((each, _), _)| *each == kind)
            .map(|(_, record)| record.digest)
            .collect()
    };
    for kind in [GROUP, RecordKind::Tag, RecordKind::Category] {
        assert_eq!(keyless(&fresh, kind), keyless(&small, kind), "{kind:?}");
    }
}

#[test]
fn the_own_record_digest_enters_no_root() {
    let document = base();
    let hashes = hash_document(&document).unwrap();
    let roots = hashes.roots;
    let mut computed: BTreeSet<Digest> = [
        roots.edges,
        roots.tags,
        roots.categories,
        roots.decisions,
        roots.content,
        roots.bindings,
        roots.state,
    ]
    .into_iter()
    .chain(roots.terms)
    .chain(roots.extensions)
    .collect();
    computed.extend(hashes.nodes.values());
    computed.extend(hashes.decisions.values());
    computed.extend(hashes.products);
    computed.extend(hashes.components);
    for record in own(&document).values() {
        assert!(!computed.contains(&record.digest), "{:?}", record.view);
    }
}

proptest! {
    #[test]
    fn own_records_never_panics_and_survives_a_write_and_a_read(
        file in 0..FIXTURES.len(),
        grown in proptest::collection::vec(growth(), 0..=12),
        edits in proptest::collection::vec(edit(), 0..=6),
    ) {
        let fixture = FIXTURES[file];
        let path = SpecPath::new(fixture.path);
        let mut files = fixture_files().clone();
        let bytes = edited(fixture.top, &files[&path], Side::Base, &grown);
        let bytes = edited(fixture.top, &bytes, Side::Ours, &edits);
        files.insert(path, bytes);
        if let Ok(formatted) = loftspec::fmt::format(&files) {
            files.extend(formatted.changed);
        }
        let read = codec::read(&files);
        prop_assume!(read.is_ok());
        let document = read.unwrap();
        let records = own(&document);
        let again = codec::read(&codec::write(&document)).unwrap();
        prop_assert_eq!(own(&again), records);
    }
}
