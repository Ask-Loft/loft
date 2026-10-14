//! Each closed set of values has one spelling: the wire name `as_str` gives is the name the
//! model's serde derive reads and writes, and every name parses back to its value.

use loftspec::attributes::{
    BandKind, ConservationKind, QaKind, QualityAggregation, QualityBand, VerificationFate,
};
use loftspec::codec::schema::{Field, Item, Kind, Scalar};
use loftspec::decision::{ConsequenceKind, DriverKind, Rating, StateKind};
use loftspec::document::SubjectRole;
use loftspec::statement::Form;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// Every value parses back from its name, and the names are listed in the values' order.
macro_rules! assert_round_trip {
    ($($name:ty),+) => {$(
        let names: Vec<&str> = <$name>::ALL.iter().map(|value| value.as_str()).collect();
        assert_eq!(names, <$name>::NAMES, stringify!($name));
        for value in <$name>::ALL {
            assert_eq!(<$name>::parse(value.as_str()), Some(*value), stringify!($name));
        }
        assert_eq!(<$name>::parse("no-such-name"), None, stringify!($name));
    )+};
}

fn read<T: DeserializeOwned>(name: &str) -> T {
    serde_json::from_value(Value::String(name.to_owned())).unwrap()
}

#[test]
fn every_wire_name_parses_back_to_its_value() {
    assert_round_trip!(
        QaKind,
        QualityAggregation,
        ConservationKind,
        VerificationFate,
        BandKind,
        Rating,
        ConsequenceKind,
        StateKind,
        DriverKind,
        SubjectRole,
        Form
    );
}

#[test]
fn every_wire_name_is_the_serde_name() {
    macro_rules! both_ways {
        ($($name:ty),+) => {$(
            for value in <$name>::ALL {
                assert_eq!(serde_json::to_value(value).unwrap(), json!(value.as_str()));
                assert_eq!(read::<$name>(value.as_str()), *value);
            }
        )+};
    }
    both_ways!(
        QaKind,
        QualityAggregation,
        ConservationKind,
        VerificationFate
    );
    for value in Rating::ALL {
        assert_eq!(read::<Rating>(value.as_str()), *value);
    }
    for value in ConsequenceKind::ALL {
        assert_eq!(read::<ConsequenceKind>(value.as_str()), *value);
    }
    for value in SubjectRole::ALL {
        assert_eq!(read::<SubjectRole>(value.as_str()), *value);
    }
}

#[test]
fn a_band_kind_is_the_kind_its_band_serializes_with() {
    let bound = || "1".to_owned();
    let bands = [
        QualityBand::AtMost { upper: bound() },
        QualityBand::AtLeast { lower: bound() },
        QualityBand::Exactly { value: bound() },
        QualityBand::Between {
            lower: bound(),
            upper: bound(),
        },
    ];
    assert_eq!(bands.len(), BandKind::ALL.len());
    for band in bands {
        let written = serde_json::to_value(&band).unwrap();
        assert_eq!(written["kind"], json!(band.kind().as_str()));
    }
}

#[test]
fn a_forms_condition_is_its_first_slot_when_it_has_three() {
    for form in Form::ALL {
        let slots = form.slots();
        assert_eq!(&slots[slots.len() - 2..], ["system", "shall"]);
        assert_eq!(form.condition(), (slots.len() == 3).then(|| slots[0]));
    }
}

/// Every field of every record kind but the counters, whose names are the label prefixes.
fn defined_fields() -> impl Iterator<Item = &'static Field> {
    Kind::ALL
        .into_iter()
        .filter(|kind| *kind != Kind::Counters)
        .flat_map(Kind::fields)
}

#[test]
fn every_field_name_the_format_defines_is_camel_case() {
    let camel = |name: &str| {
        name.starts_with(|first: char| first.is_ascii_lowercase())
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
    };
    let names: Vec<&str> = defined_fields().map(|field| field.name).collect();
    let other: Vec<&&str> = names.iter().filter(|name| !camel(name)).collect();
    assert!(other.is_empty(), "{other:?}");
    assert!(names.contains(&"qualityTarget"), "{names:?}");
}

#[test]
fn every_enumerated_value_the_format_defines_is_snake_case() {
    let snake = |value: &str| {
        value.starts_with(|first: char| first.is_ascii_lowercase())
            && value.split('_').all(|word| {
                !word.is_empty()
                    && word.chars().all(|character| {
                        character.is_ascii_lowercase() || character.is_ascii_digit()
                    })
            })
    };
    let values: Vec<&str> = defined_fields()
        .filter_map(|field| match field.shape.item() {
            Item::Scalar(Scalar::Enum(values)) => Some(values),
            _ => None,
        })
        .flatten()
        .copied()
        .collect();
    let other: Vec<&&str> = values.iter().filter(|value| !snake(value)).collect();
    assert!(other.is_empty(), "{other:?}");
    assert!(values.contains(&"event_driven"), "{values:?}");
}
