//! Bindings and aliases (D-11 F-2): the publisher's relabel records the previous label as a
//! permanent alias, and the validator refuses aliases that collide.

use loftspec::bindings::{BindingError, Bindings};
use loftspec::identity::{Key, Label};

fn key(n: u8) -> Key {
    Key::parse(&format!("00000000-0000-7000-8000-0000000000{n:02x}")).unwrap()
}

fn label(text: &str) -> Label {
    Label::parse(text).unwrap()
}

#[test]
fn ac_20_a_bound_key_has_exactly_one_label() {
    let mut bindings = Bindings::default();
    bindings.bind(key(1), label("RQ-1")).unwrap();
    assert_eq!(bindings.label_of(key(1)), Some(label("RQ-1")));
    assert_eq!(
        bindings.bind(key(1), label("RQ-2")),
        Err(BindingError::AlreadyBound(key(1)))
    );
    assert_eq!(bindings.label_of(key(1)), Some(label("RQ-1")));
    assert_eq!(bindings.len(), 1);
}

#[test]
fn ac_27_a_relabel_records_the_previous_label_as_an_alias() {
    let mut bindings = Bindings::default();
    bindings.bind(key(1), label("RQ-7")).unwrap();
    bindings.relabel(key(1), label("F-3")).unwrap();
    assert_eq!(bindings.label_of(key(1)), Some(label("F-3")));
    assert_eq!(bindings.aliases_of(key(1)), [label("RQ-7")]);
}

#[test]
fn ac_28_an_alias_list_only_grows() {
    let mut bindings = Bindings::default();
    bindings.bind(key(1), label("RQ-7")).unwrap();
    bindings.relabel(key(1), label("F-3")).unwrap();
    bindings.relabel(key(1), label("FG-2")).unwrap();
    assert_eq!(bindings.aliases_of(key(1)), [label("RQ-7"), label("F-3")]);
    bindings.relabel(key(1), label("FG-2")).unwrap();
    assert_eq!(bindings.aliases_of(key(1)), [label("RQ-7"), label("F-3")]);
}

#[test]
fn ac_29_an_alias_resolves_to_the_key_it_was_assigned_to() {
    let mut bindings = Bindings::default();
    bindings.bind(key(1), label("RQ-7")).unwrap();
    bindings.bind(key(2), label("RQ-8")).unwrap();
    bindings.relabel(key(1), label("F-3")).unwrap();
    assert_eq!(bindings.resolve(label("RQ-7")), Some(key(1)));
    assert_eq!(bindings.resolve(label("F-3")), Some(key(1)));
    assert_eq!(bindings.resolve(label("RQ-8")), Some(key(2)));
    assert_eq!(bindings.resolve(label("RQ-9")), None);
}

#[test]
fn relabelling_an_unbound_key_is_refused() {
    let mut bindings = Bindings::default();
    assert_eq!(
        bindings.relabel(key(1), label("F-3")),
        Err(BindingError::Unbound(key(1)))
    );
}

#[test]
fn ac_32_and_ac_33_an_alias_equal_to_another_keys_label_names_both_keys_and_the_label() {
    let mut bindings = Bindings::default();
    bindings.bind(key(1), label("RQ-7")).unwrap();
    bindings.relabel(key(1), label("F-3")).unwrap();
    bindings.bind(key(2), label("RQ-7")).unwrap();
    let errors = bindings.validate();
    assert_eq!(
        errors,
        [BindingError::AliasIsLiveLabel {
            alias: label("RQ-7"),
            alias_key: key(1),
            label_key: key(2)
        }]
    );
    let message = errors[0].to_string();
    for part in [key(1).to_string(), key(2).to_string(), "RQ-7".to_owned()] {
        assert!(message.contains(&part), "{message} names {part}");
    }
}

#[test]
fn ac_232_and_ac_233_an_alias_under_two_keys_names_both_keys_and_the_alias() {
    let mut bindings = Bindings::default();
    bindings.bind(key(1), label("RQ-7")).unwrap();
    bindings.relabel(key(1), label("F-3")).unwrap();
    bindings.bind(key(2), label("RQ-9")).unwrap();
    bindings.add_alias(key(2), label("RQ-7")).unwrap();
    let errors = bindings.validate();
    assert_eq!(
        errors,
        [BindingError::AliasUnderTwoKeys {
            alias: label("RQ-7"),
            first: key(1),
            second: key(2)
        }]
    );
    let message = errors[0].to_string();
    for part in [key(1).to_string(), key(2).to_string(), "RQ-7".to_owned()] {
        assert!(message.contains(&part), "{message} names {part}");
    }
}

#[test]
fn distinct_labels_and_aliases_validate_cleanly() {
    let mut bindings = Bindings::default();
    bindings.bind(key(1), label("RQ-7")).unwrap();
    bindings.relabel(key(1), label("F-3")).unwrap();
    bindings.bind(key(2), label("RQ-8")).unwrap();
    assert_eq!(bindings.validate(), []);
}
