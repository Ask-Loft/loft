//! Keys and labels (D-11 F-1 and F-2). Criterion labels are those of the Authored State pinned by
//! `docs/loft.lock.json`.

use loftspec::identity::{Key, KeyError, Label, LabelError, LabelPrefix};
use loftspec::kind::NodeKind;

const V4: &str = "4b0f3088-45c1-4a26-a553-9b0039879c9b";

#[test]
fn ac_2_a_key_is_written_in_lowercase_hyphenated_form() {
    let key = Key::parse(V4).expect("a lowercase hyphenated UUID is a key");
    assert_eq!(key.to_string(), V4);
}

#[test]
fn ac_3_and_ac_4_a_key_of_any_version_reads_back_unchanged() {
    for text in [
        V4,
        "00000000-0000-0000-0000-000000000000",
        "0192a0b4-6f3c-7c1e-9d2a-3b4c5d6e7f80",
        "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
    ] {
        assert_eq!(Key::parse(text).unwrap().to_string(), text);
    }
}

#[test]
fn ac_10_a_minted_key_is_version_7() {
    assert_eq!(Key::mint().version(), 7);
}

#[test]
fn ac_11_two_keys_minted_in_one_run_differ() {
    let first = Key::mint();
    let second = Key::mint();
    assert_ne!(first, second);
    assert!(first < second, "version 7 keys minted later sort later");
}

#[test]
fn ac_210_and_ac_212_an_uppercase_key_is_an_error_naming_it() {
    let text = "4B0F3088-45C1-4A26-A553-9B0039879C9B";
    let error = Key::parse(text).unwrap_err();
    assert_eq!(error, KeyError::Uppercase(text.to_owned()));
    assert!(error.to_string().contains(text));
}

#[test]
fn ac_213_a_key_without_hyphens_is_an_error() {
    let text = "4b0f308845c14a26a5539b0039879c9b";
    assert_eq!(
        Key::parse(text),
        Err(KeyError::NotHyphenated(text.to_owned()))
    );
}

#[test]
fn ac_210_a_key_that_is_not_a_uuid_is_an_error_naming_it() {
    for text in [
        "",
        "rq_8f3k2",
        "4b0f3088-45c1-4a26-a553-9b0039879c9",
        "{4b0f3088-45c1-4a26-a553-9b0039879c9b}",
    ] {
        assert_eq!(Key::parse(text), Err(KeyError::NotUuid(text.to_owned())));
    }
}

#[test]
fn ac_22_a_label_is_a_prefix_a_hyphen_and_a_positive_integer() {
    let label = Label::parse("RQ-23").unwrap();
    assert_eq!(label.prefix(), LabelPrefix::Requirement);
    assert_eq!(label.number().get(), 23);
    assert_eq!(label.to_string(), "RQ-23");
}

#[test]
fn ac_22_a_label_number_has_no_leading_zero_and_is_positive() {
    for text in ["RQ-023", "RQ-0", "RQ-", "RQ-1a", "RQ--1", "RQ23", "RQ-+1"] {
        assert_eq!(
            Label::parse(text),
            Err(LabelError::Malformed(text.to_owned())),
            "{text}"
        );
    }
}

#[test]
fn ac_243_the_kind_prefixes_are_closed() {
    let expected = [
        ("P", NodeKind::Product),
        ("FG", NodeKind::FeatureGroup),
        ("F", NodeKind::Feature),
        ("RQ", NodeKind::Requirement),
        ("AC", NodeKind::Criterion),
        ("CMP", NodeKind::Component),
        ("CON", NodeKind::Constraint),
        ("QA", NodeKind::QualityAttribute),
    ];
    for (prefix, kind) in expected {
        let parsed = Label::parse(&format!("{prefix}-1")).unwrap().prefix();
        assert_eq!(parsed.as_str(), prefix);
        assert_eq!(kind.label_prefix(), Some(parsed));
    }
    assert_eq!(
        Label::parse("ADR-6").unwrap().prefix(),
        LabelPrefix::Decision
    );
    assert_eq!(NodeKind::Section.label_prefix(), None);
    assert_eq!(
        Label::parse("FT-1"),
        Err(LabelError::UnknownPrefix("FT-1".to_owned()))
    );
}
