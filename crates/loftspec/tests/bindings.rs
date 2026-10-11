//! Bindings and aliases (D-11 F-2): a label changes only by relabelling, which keeps the previous
//! label as a permanent alias, and the validator refuses aliases that collide.

mod common;

use common::{key, label};
use loftspec::bindings::{BindingError, Bindings};
use loftspec::ids::{ElementId, FeatureId, ProductId, RequirementId, SectionId};

fn requirement(n: u32) -> RequirementId {
    RequirementId::new(key(n))
}

#[test]
fn ac_20_a_bound_element_has_exactly_one_label() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(1), label("RQ-1")).unwrap();
    assert_eq!(bindings.label_of(requirement(1)), Some(label("RQ-1")));
    assert_eq!(
        bindings.bind(requirement(1), label("RQ-2")),
        Err(BindingError::AlreadyBound(requirement(1).into()))
    );
    assert_eq!(bindings.label_of(requirement(1)), Some(label("RQ-1")));
    assert_eq!(bindings.len(), 1);
}

#[test]
fn ac_243_a_label_carries_its_elements_kind_prefix() {
    let mut bindings = Bindings::default();
    assert_eq!(
        bindings.bind(ProductId::new(key(1)), label("RQ-1")),
        Err(BindingError::PrefixMismatch {
            element: ProductId::new(key(1)).into(),
            label: label("RQ-1")
        })
    );
    assert!(matches!(
        bindings.bind(SectionId::new(key(2)), label("P-2")),
        Err(BindingError::PrefixMismatch { .. })
    ));
    assert!(bindings.is_empty());
}

#[test]
fn ac_27_a_relabel_records_the_previous_label_as_an_alias() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(1), label("RQ-7")).unwrap();
    bindings.relabel(requirement(1), label("RQ-9")).unwrap();
    assert_eq!(bindings.label_of(requirement(1)), Some(label("RQ-9")));
    assert_eq!(bindings.aliases_of(requirement(1)), [label("RQ-7")]);
}

#[test]
fn ac_28_an_alias_list_only_grows() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(1), label("RQ-7")).unwrap();
    bindings.relabel(requirement(1), label("RQ-8")).unwrap();
    bindings.relabel(requirement(1), label("RQ-9")).unwrap();
    assert_eq!(
        bindings.aliases_of(requirement(1)),
        [label("RQ-7"), label("RQ-8")]
    );
    bindings.relabel(requirement(1), label("RQ-9")).unwrap();
    assert_eq!(
        bindings.aliases_of(requirement(1)),
        [label("RQ-7"), label("RQ-8")]
    );
}

#[test]
fn ac_31_and_ac_224_a_promoted_requirement_keeps_its_key_and_its_label_as_an_alias() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(1), label("RQ-7")).unwrap();
    let feature: FeatureId = requirement(1).promote();
    assert_eq!(feature.key(), requirement(1).key());
    bindings.relabel(feature, label("F-3")).unwrap();
    assert_eq!(bindings.resolve(label("F-3")), Some(feature.into()));
    assert_eq!(bindings.resolve(label("RQ-7")), Some(feature.into()));
    assert_eq!(bindings.aliases_of(feature), [label("RQ-7")]);
}

#[test]
fn ac_29_an_alias_resolves_to_the_element_it_was_assigned_to() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(1), label("RQ-7")).unwrap();
    bindings.bind(requirement(2), label("RQ-8")).unwrap();
    bindings.relabel(requirement(1), label("RQ-9")).unwrap();
    let resolve = |text: &str| bindings.resolve(label(text));
    assert_eq!(resolve("RQ-7"), Some(ElementId::from(requirement(1))));
    assert_eq!(resolve("RQ-9"), Some(ElementId::from(requirement(1))));
    assert_eq!(resolve("RQ-8"), Some(ElementId::from(requirement(2))));
    assert_eq!(resolve("RQ-10"), None);
}

#[test]
fn relabelling_an_unbound_element_is_refused() {
    let mut bindings = Bindings::default();
    assert_eq!(
        bindings.relabel(requirement(1), label("RQ-3")),
        Err(BindingError::Unbound(requirement(1).into()))
    );
}

#[test]
fn ac_32_and_ac_33_an_alias_equal_to_another_elements_label_names_both_and_the_label() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(1), label("RQ-7")).unwrap();
    bindings.relabel(requirement(1), label("RQ-9")).unwrap();
    bindings.bind(requirement(2), label("RQ-7")).unwrap();
    let errors = bindings.validate();
    assert_eq!(
        errors,
        [BindingError::AliasIsLiveLabel {
            alias: label("RQ-7"),
            alias_element: requirement(1).into(),
            label_element: requirement(2).into(),
        }]
    );
    let message = errors[0].to_string();
    for part in [key(1).to_string(), key(2).to_string(), "RQ-7".to_owned()] {
        assert!(message.contains(&part), "{message} names {part}");
    }
}

#[test]
fn ac_232_and_ac_233_an_alias_under_two_elements_names_both_and_the_alias() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(1), label("RQ-7")).unwrap();
    bindings.relabel(requirement(1), label("RQ-9")).unwrap();
    bindings.bind(requirement(2), label("RQ-8")).unwrap();
    bindings.add_alias(requirement(2), label("RQ-7")).unwrap();
    let errors = bindings.validate();
    assert_eq!(
        errors,
        [BindingError::AliasUnderTwoKeys {
            alias: label("RQ-7"),
            first: requirement(1).into(),
            second: requirement(2).into(),
        }]
    );
    let message = errors[0].to_string();
    for part in [key(1).to_string(), key(2).to_string(), "RQ-7".to_owned()] {
        assert!(message.contains(&part), "{message} names {part}");
    }
}

#[test]
fn ac_287_and_ac_288_two_keys_with_one_live_label_name_both_and_the_label() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(1), label("RQ-7")).unwrap();
    bindings.bind(requirement(2), label("RQ-7")).unwrap();
    let errors = bindings.validate();
    assert_eq!(
        errors,
        [BindingError::LabelUnderTwoKeys {
            label: label("RQ-7"),
            first: requirement(1).into(),
            second: requirement(2).into(),
        }]
    );
    let message = errors[0].to_string();
    for part in [key(1).to_string(), key(2).to_string(), "RQ-7".to_owned()] {
        assert!(message.contains(&part), "{message} names {part}");
    }
}

#[test]
fn ac_289_distinct_labels_and_aliases_validate_cleanly() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(1), label("RQ-7")).unwrap();
    bindings.relabel(requirement(1), label("RQ-9")).unwrap();
    bindings.bind(requirement(2), label("RQ-8")).unwrap();
    assert_eq!(bindings.validate(), []);
}

#[test]
fn a_live_label_resolves_before_an_alias_and_the_lowest_key_resolves_a_clash() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(3), label("RQ-7")).unwrap();
    bindings.relabel(requirement(3), label("RQ-9")).unwrap();
    bindings.bind(requirement(4), label("RQ-7")).unwrap();
    bindings.bind(requirement(2), label("RQ-5")).unwrap();
    bindings.bind(requirement(1), label("RQ-5")).unwrap();
    assert_eq!(
        bindings.resolve(label("RQ-7")),
        Some(requirement(4).into()),
        "RQ-7 is the live label of RQ-4's key and an alias of another"
    );
    assert_eq!(bindings.resolve(label("RQ-5")), Some(requirement(1).into()));
    bindings.relabel(requirement(1), label("RQ-6")).unwrap();
    assert_eq!(bindings.resolve(label("RQ-5")), Some(requirement(2).into()));
}
