//! What one take of the outbox makes a batch of, by count and by bytes; the reason kept beside
//! each file moved into `dead/`; and the survey of what waits, which moves nothing.

use std::fs;

use serde_json::json;

use yardlog::batch::{MOST_BYTES, MOST_RECORDS};
use yardlog_store::outbox::{self, DEAD};

use crate::common::Scratch;
use crate::outbox::{ENDPOINT, entry, outbox};

/// The name of the entry file of the version `n`.
fn name(n: u32) -> String {
    format!("00000000-0000-7000-8000-{n:012}.json")
}

/// Puts the entry of version `n` for `endpoint`, padded to about `bytes` bytes: the entry holds
/// its id twice, as the record's id and as its version's key.
fn put(scratch: &Scratch, endpoint: &str, n: u32, bytes: usize) {
    let id = format!("{n}{}", "x".repeat(bytes / 2));
    scratch.put(
        &format!(".yard/outbox/{}", name(n)),
        &entry(endpoint, &id, 1),
    );
}

/// The keys a take of at most 16 entries and 1 MiB gives.
fn taken(scratch: &Scratch) -> Vec<String> {
    outbox(scratch, ENDPOINT)
        .take(MOST_RECORDS, MOST_BYTES)
        .unwrap()
        .entries
        .into_iter()
        .map(|entry| entry.key)
        .collect()
}

fn key(n: u32) -> String {
    name(n).trim_end_matches(".json").to_owned()
}

/// The reason kept beside the file `name` in `dead/`, where there is one.
fn reason(scratch: &Scratch, name: &str) -> Option<String> {
    fs::read_to_string(scratch.0.join(format!(".yard/outbox/{DEAD}/{name}.reason"))).ok()
}

#[test]
fn an_entry_that_would_bring_a_batch_over_1_mib_goes_in_the_next_batch() {
    let scratch = Scratch::new();
    let third = 400 * 1024;
    for n in 1..=3 {
        put(&scratch, ENDPOINT, n, third);
    }
    put(&scratch, ENDPOINT, 4, 10);
    assert_eq!(taken(&scratch), [key(1), key(2)]);
}

#[test]
fn an_entry_larger_than_1_mib_goes_in_a_batch_of_its_own() {
    let scratch = Scratch::new();
    put(&scratch, ENDPOINT, 1, 10);
    put(&scratch, ENDPOINT, 2, 1536 * 1024);
    put(&scratch, ENDPOINT, 3, 10);
    assert_eq!(taken(&scratch), [key(1)]);
    fs::remove_file(scratch.0.join(".yard/outbox").join(name(1))).unwrap();
    assert_eq!(taken(&scratch), [key(2)]);
}

#[test]
fn seventeen_small_entries_make_a_batch_of_the_16_oldest() {
    let scratch = Scratch::new();
    for n in 1..=17 {
        put(&scratch, ENDPOINT, n, 10);
    }
    let keys = taken(&scratch);
    assert_eq!(keys.len(), 16);
    assert_eq!(keys.first(), Some(&key(1)));
    assert_eq!(keys.last(), Some(&key(16)));
}

#[test]
fn a_file_that_is_not_an_entry_is_moved_into_dead_with_the_reason_not_an_entry() {
    let scratch = Scratch::new();
    scratch.put(&format!(".yard/outbox/{}", name(1)), "{not json");
    outbox(&scratch, ENDPOINT)
        .take(MOST_RECORDS, MOST_BYTES)
        .unwrap();
    assert!(scratch.0.join(".yard/outbox/dead").join(name(1)).is_file());
    assert_eq!(
        reason(&scratch, &name(1)).as_deref(),
        Some("not-an-entry\n")
    );
}

#[test]
fn an_entry_whose_record_holds_no_version_key_is_moved_into_dead_and_the_others_are_taken() {
    let scratch = Scratch::new();
    put(&scratch, ENDPOINT, 1, 10);
    let keyless = json!({
        "format": "yardlog-outbox/1",
        "to": {"endpoint": ENDPOINT},
        "record": {"record": "version", "id": "2", "version": {}},
    });
    scratch.put(&format!(".yard/outbox/{}", name(2)), &keyless.to_string());
    put(&scratch, ENDPOINT, 3, 10);
    assert_eq!(taken(&scratch), [key(1), key(3)]);
    assert_eq!(
        reason(&scratch, &name(2)).as_deref(),
        Some("not-an-entry\n")
    );
}

#[test]
fn an_entry_moved_aside_keeps_its_reason_beside_it() {
    let scratch = Scratch::new();
    for n in 1..=3 {
        put(&scratch, ENDPOINT, n, 10);
    }
    let ours = outbox(&scratch, ENDPOINT);
    let entries = ours.take(MOST_RECORDS, MOST_BYTES).unwrap().entries;
    for (entry, why) in entries.iter().zip([
        "conflict",
        "refused invalid_record",
        "413 payload_too_large",
    ]) {
        assert!(ours.bury(entry, why).unwrap());
    }
    assert_eq!(reason(&scratch, &name(1)).as_deref(), Some("conflict\n"));
    assert_eq!(
        reason(&scratch, &name(2)).as_deref(),
        Some("refused invalid_record\n")
    );
    assert_eq!(
        reason(&scratch, &name(3)).as_deref(),
        Some("413 payload_too_large\n")
    );
    assert_eq!(taken(&scratch), Vec::<String>::new());
}

#[test]
fn a_reason_file_is_never_taken_as_an_entry_and_never_moved() {
    let scratch = Scratch::new();
    let stray = format!("{}.reason", name(1));
    scratch.put(&format!(".yard/outbox/{stray}"), "conflict\n");
    put(&scratch, ENDPOINT, 2, 10);
    let found = outbox(&scratch, ENDPOINT)
        .take(MOST_RECORDS, MOST_BYTES)
        .unwrap();
    assert_eq!((found.entries.len(), found.dead), (1, 0));
    assert!(scratch.0.join(".yard/outbox").join(&stray).is_file());
}

#[test]
fn a_survey_counts_what_waits_for_each_address_and_moves_nothing() {
    let scratch = Scratch::new();
    put(&scratch, ENDPOINT, 1, 10);
    put(&scratch, ENDPOINT, 2, 10);
    put(&scratch, "https://elsewhere.example", 3, 10);
    scratch.put(
        &format!(".yard/outbox/{}", name(4)),
        &entry(ENDPOINT, "4", 2),
    );
    scratch.put(&format!(".yard/outbox/{}", name(5)), "{not json");
    let before = scratch.snapshot();
    let survey = outbox::survey(&scratch.0.join(".yard/outbox")).unwrap();
    assert_eq!(survey.waiting[ENDPOINT], [key(1), key(2)]);
    assert_eq!(survey.waiting["https://elsewhere.example"], [key(3)]);
    assert_eq!(survey.newer, 1);
    assert_eq!(scratch.snapshot(), before);
}

#[test]
fn a_missing_outbox_has_nothing_to_survey() {
    let scratch = Scratch::new();
    let survey = outbox::survey(&scratch.0.join(".yard/outbox")).unwrap();
    assert!(survey.waiting.is_empty());
    assert_eq!(survey.newer, 0);
}
