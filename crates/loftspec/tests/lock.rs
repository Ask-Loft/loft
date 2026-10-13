//! The lock record (RQ-19, F-29): its fields and layout, what makes it valid, and its time.

use std::num::NonZeroU64;

use loftspec::canonical::Digest;
use loftspec::document::DerivedFrom;
use loftspec::identity::Key;
use loftspec::ids::{AuthoredStateId, DocumentId};
use loftspec::publish::{Lock, LockError, PublishedAt, TimeError};

fn digest(byte: char) -> Digest {
    byte.to_string().repeat(64).parse().unwrap()
}

fn lock(derived: bool) -> Lock {
    Lock {
        document_key: DocumentId::new(Key::parse("31c36546-9554-426c-980b-a501a32aa506").unwrap()),
        state_root: digest('a'),
        content_root: digest('b'),
        binding_root: digest('c'),
        publish_sequence: NonZeroU64::new(2).unwrap(),
        published_at: PublishedAt::from_unix_seconds(1_760_227_200).unwrap(),
        derived_from: derived.then(|| DerivedFrom {
            authored_state_id: AuthoredStateId::new(
                Key::parse("a0654aa3-9372-4a39-bba5-dffa80ef625f").unwrap(),
            ),
            content_address: digest('d'),
            publish_sequence: 3,
        }),
    }
}

const DERIVED: &str = r#"{
  "format": "loftspec/1",
  "algorithm": "sha256-jcs/1",
  "documentKey": "31c36546-9554-426c-980b-a501a32aa506",
  "stateRoot": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "contentRoot": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "bindingRoot": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
  "publishSequence": 2,
  "publishedAt": "2025-10-12T00:00:00Z",
  "derivedFrom": {
    "authoredStateId": "a0654aa3-9372-4a39-bba5-dffa80ef625f",
    "contentAddress": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    "publishSequence": 3
  }
}
"#;

#[test]
fn the_lock_carries_its_fields_in_the_formats_order_indented_by_two_spaces() {
    assert_eq!(String::from_utf8(lock(true).to_bytes()).unwrap(), DERIVED);
}

#[test]
fn a_lock_of_a_document_that_is_not_derived_has_no_derived_from_field() {
    let text = String::from_utf8(lock(false).to_bytes()).unwrap();
    assert!(!text.contains("derivedFrom"), "{text}");
    assert!(
        text.ends_with("\"publishedAt\": \"2025-10-12T00:00:00Z\"\n}\n"),
        "{text}"
    );
}

#[test]
fn a_lock_reads_back_as_written() {
    for derived in [true, false] {
        assert_eq!(
            Lock::read(&lock(derived).to_bytes()).unwrap(),
            lock(derived)
        );
    }
}

fn without(field: &str) -> String {
    DERIVED
        .lines()
        .filter(|line| !line.trim_start().starts_with(&format!("\"{field}\"")))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_lock_that_lacks_a_lock_field_is_not_valid() {
    for field in [
        "format",
        "algorithm",
        "documentKey",
        "stateRoot",
        "contentRoot",
        "bindingRoot",
        "publishedAt",
    ] {
        let error = Lock::read(without(field).as_bytes()).unwrap_err();
        assert!(error.to_string().contains(field), "{field}: {error}");
    }
}

#[test]
fn a_lock_that_holds_another_field_is_not_valid() {
    let text = DERIVED.replacen("{\n", "{\n  \"labels\": 3,\n", 1);
    let error = Lock::read(text.as_bytes()).unwrap_err();
    assert!(error.to_string().contains("labels"), "{error}");
}

#[test]
fn a_lock_that_names_an_unknown_format_or_algorithm_is_not_valid() {
    let format = DERIVED.replace("loftspec/1", "loftspec/2");
    assert!(matches!(
        Lock::read(format.as_bytes()),
        Err(LockError::Identifier(_))
    ));
    let algorithm = DERIVED.replace("sha256-jcs/1", "blake3/1");
    assert!(matches!(
        Lock::read(algorithm.as_bytes()),
        Err(LockError::Identifier(_))
    ));
}

#[test]
fn a_lock_with_a_malformed_value_is_not_valid() {
    for (from, to) in [
        ("\"stateRoot\": \"aaaa", "\"stateRoot\": \"AAAA"),
        ("\"publishSequence\": 2", "\"publishSequence\": 0"),
        ("\"publishSequence\": 2", "\"publishSequence\": \"2\""),
        ("2025-10-12T00:00:00Z", "2025-10-12 00:00:00Z"),
        ("2025-10-12T00:00:00Z", "2025-02-29T00:00:00Z"),
        ("31c36546-9554", "31C36546-9554"),
        ("{\n  \"format\"", "[\n  \"format\""),
    ] {
        let text = DERIVED.replacen(from, to, 1);
        assert_ne!(text, DERIVED, "{to}");
        assert!(Lock::read(text.as_bytes()).is_err(), "{to}");
    }
}

#[test]
fn the_publish_time_is_written_in_utc_to_the_second() {
    for (seconds, text) in [
        (0, "1970-01-01T00:00:00Z"),
        (951_782_400, "2000-02-29T00:00:00Z"),
        (1_709_164_799, "2024-02-28T23:59:59Z"),
        (1_709_251_199, "2024-02-29T23:59:59Z"),
        (4_107_542_400, "2100-03-01T00:00:00Z"),
        (253_402_300_799, "9999-12-31T23:59:59Z"),
    ] {
        let time = PublishedAt::from_unix_seconds(seconds).unwrap();
        assert_eq!(time.to_string(), text);
        assert_eq!(text.parse::<PublishedAt>().unwrap(), time, "{text}");
    }
}

#[test]
fn a_time_past_the_year_9999_cannot_be_written() {
    assert_eq!(
        PublishedAt::from_unix_seconds(253_402_300_800),
        Err(TimeError::TooLate(253_402_300_800))
    );
}

#[test]
fn a_time_that_is_not_a_utc_second_is_refused() {
    for text in [
        "1969-12-31T23:59:59Z",
        "2026-13-01T00:00:00Z",
        "2026-04-31T00:00:00Z",
        "2100-02-29T00:00:00Z",
        "2026-10-12T24:00:00Z",
        "2026-10-12T00:00:00+00:00",
        "2026-10-12T00:00:00.5Z",
    ] {
        assert!(text.parse::<PublishedAt>().is_err(), "{text}");
    }
}
