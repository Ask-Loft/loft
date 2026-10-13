//! The merge of `spec/labels.yaml` when each side published (F-32, F-60): counters take the higher value, and the bindings two publishes gave apart leave
//! the merged bindings for one conflict record over `bindings`.

mod common;

use common::text::{files, text};
use loftspec::codec::SpecPath;
use loftspec::merge::{Merged, merge_file};

/// The small document's `spec/labels.yaml`.
fn labels_file() -> String {
    text(&files(), "labels.yaml")
}

/// `labels` with the counter of `prefix` set to `next`.
fn counter(labels: &str, prefix: &str, next: u64) -> String {
    let line = format!("  {prefix}: ");
    let start = labels.find(&line).unwrap_or_else(|| panic!("{line:?}"));
    let end = start + labels[start..].find('\n').unwrap();
    format!("{}{line}{next}{}", &labels[..start], &labels[end..])
}

/// A binding of key `0…0a<n>`, which sorts after the small document's keys.
fn binding(n: u32, label: &str, aliases: &[&str], withdrawn: &[&str]) -> String {
    let list = |name: &str, labels: &[&str]| {
        if labels.is_empty() {
            String::new()
        } else {
            let items = labels.join("\n      - ");
            format!("    {name}:\n      - {items}\n")
        }
    };
    format!(
        "  - key: 00000000-0000-7000-8000-0000000000a{n}\n    label: {label}\n{}{}",
        list("aliases", aliases),
        list("withdrawn", withdrawn)
    )
}

/// `labels` with `bindings` appended to its bindings, which end the file.
fn with(labels: &str, bindings: &[String]) -> String {
    format!("{labels}{}", bindings.concat())
}

fn merge(base: &str, ours: &str, theirs: &str) -> (String, Merged) {
    let merged = merge_file(
        &SpecPath::new("labels.yaml"),
        base.as_bytes(),
        ours.as_bytes(),
        theirs.as_bytes(),
    )
    .expect("the labels merge");
    (String::from_utf8(merged.bytes.clone()).unwrap(), merged)
}

/// The labels the merge reports as disputed, as text.
fn disputed(merged: &Merged) -> Vec<String> {
    merged.disputed.iter().map(ToString::to_string).collect()
}

/// The merged text up to its conflict records.
fn bindings_part(text: &str) -> &str {
    text.split("conflicts:\n").next().unwrap()
}

#[test]
fn each_counter_takes_the_higher_value_without_a_conflict() {
    let base = labels_file();
    let ours = counter(&counter(&base, "RQ", 30), "AC", 9);
    let theirs = counter(&counter(&base, "RQ", 31), "AC", 7);
    let (text, merged) = merge(&base, &ours, &theirs);
    assert!(
        text.contains("  RQ: 31\n") && text.contains("  AC: 9\n"),
        "{text}"
    );
    assert_eq!(merged.conflicts, 0);
}

#[test]
fn bindings_two_publishes_give_apart_without_colliding_are_both_kept() {
    let base = labels_file();
    let ours = with(&counter(&base, "RQ", 51), &[binding(1, "RQ-50", &[], &[])]);
    let theirs = with(&counter(&base, "RQ", 52), &[binding(2, "RQ-51", &[], &[])]);
    let (text, merged) = merge(&base, &ours, &theirs);
    assert!(
        text.ends_with(&with(
            "",
            &[binding(1, "RQ-50", &[], &[]), binding(2, "RQ-51", &[], &[])]
        )),
        "{text}"
    );
    assert_eq!((merged.conflicts, disputed(&merged)), (0, Vec::new()));
    assert!(text.contains("  RQ: 52\n"));
}

#[test]
fn a_live_label_two_keys_hold_leaves_both_bindings_out_with_one_conflict_over_bindings() {
    let base = labels_file();
    let ours = with(&counter(&base, "RQ", 51), &[binding(1, "RQ-50", &[], &[])]);
    let theirs = with(&counter(&base, "RQ", 51), &[binding(2, "RQ-50", &[], &[])]);
    let (text, merged) = merge(&base, &ours, &theirs);
    assert!(!bindings_part(&text).contains("RQ-50"), "{text}");
    let conflict = "conflicts:
  - field: bindings
    ours:
      - key: 00000000-0000-7000-8000-0000000000a1
        label: RQ-50
    theirs:
      - key: 00000000-0000-7000-8000-0000000000a2
        label: RQ-50
";
    assert!(text.ends_with(conflict), "{text}");
    assert_eq!(
        (merged.conflicts, disputed(&merged)),
        (1, vec!["RQ-50".to_owned()])
    );
}

#[test]
fn a_label_one_key_holds_live_and_another_as_an_alias_puts_both_in_dispute() {
    let base = labels_file();
    let ours = with(&counter(&base, "RQ", 61), &[binding(1, "RQ-50", &[], &[])]);
    let theirs = with(
        &counter(&base, "RQ", 61),
        &[binding(2, "RQ-60", &["RQ-50"], &[])],
    );
    let (text, merged) = merge(&base, &ours, &theirs);
    for key in ["0000000000a1", "0000000000a2"] {
        assert!(!bindings_part(&text).contains(key), "{key}: {text}");
    }
    assert_eq!(merged.conflicts, 1);
    assert!(
        disputed(&merged).contains(&"RQ-50".to_owned()),
        "{:?}",
        merged.disputed
    );
}

#[test]
fn a_key_both_sides_bind_to_different_labels_that_base_lacks_is_left_out() {
    let base = labels_file();
    let ours = with(&counter(&base, "RQ", 51), &[binding(1, "RQ-50", &[], &[])]);
    let theirs = with(&counter(&base, "RQ", 52), &[binding(1, "RQ-51", &[], &[])]);
    let (text, merged) = merge(&base, &ours, &theirs);
    assert!(!bindings_part(&text).contains("0000000000a1"), "{text}");
    assert!(
        text.contains(
            "    ours:\n      - key: 00000000-0000-7000-8000-0000000000a1\n        label: RQ-50\n"
        ),
        "{text}"
    );
    assert!(!text.contains("    base:\n"), "{text}");
    assert_eq!(merged.conflicts, 1);
}

#[test]
fn a_key_both_sides_bind_to_different_labels_that_base_holds_keeps_its_base_binding() {
    let base = with(
        &counter(&labels_file(), "RQ", 51),
        &[binding(1, "RQ-50", &[], &[])],
    );
    let ours = with(
        &counter(&labels_file(), "RQ", 52),
        &[binding(1, "RQ-51", &["RQ-50"], &[])],
    );
    let theirs = with(
        &counter(&labels_file(), "RQ", 53),
        &[binding(1, "RQ-52", &["RQ-50"], &[])],
    );
    let (text, merged) = merge(&base, &ours, &theirs);
    assert!(
        bindings_part(&text).ends_with(&binding(1, "RQ-50", &[], &[])),
        "{text}"
    );
    assert!(text.contains("    base:\n      - key: 00000000-0000-7000-8000-0000000000a1\n        label: RQ-50\n    ours:\n"), "{text}");
    assert_eq!(merged.conflicts, 1);
    assert!(text.contains("  RQ: 53\n"));
}

#[test]
fn a_withdrawn_label_puts_no_binding_in_dispute() {
    let base = labels_file();
    let ours = with(
        &counter(&base, "RQ", 52),
        &[binding(1, "RQ-51", &[], &["RQ-50"])],
    );
    let theirs = with(&counter(&base, "RQ", 51), &[binding(2, "RQ-50", &[], &[])]);
    let (_, merged) = merge(&base, &ours, &theirs);
    assert_eq!((merged.conflicts, disputed(&merged)), (0, Vec::new()));
}

#[test]
fn only_the_disputed_bindings_leave_and_the_others_merge() {
    let base = labels_file();
    let ours = with(
        &counter(&base, "RQ", 52),
        &[binding(1, "RQ-50", &[], &[]), binding(3, "RQ-51", &[], &[])],
    );
    let theirs = with(&counter(&base, "RQ", 51), &[binding(2, "RQ-50", &[], &[])]);
    let (text, merged) = merge(&base, &ours, &theirs);
    assert!(
        bindings_part(&text).ends_with(&binding(3, "RQ-51", &[], &[])),
        "{text}"
    );
    for (key, label) in [("P", "P-1"), ("RQ", "RQ-2")] {
        assert!(
            bindings_part(&text).contains(&format!("label: {label}\n")),
            "{key}: {text}"
        );
    }
    assert_eq!(merged.conflicts, 1);
}

#[test]
fn a_bindings_conflict_both_inputs_hold_is_written_once() {
    let conflict = "conflicts:
  - field: bindings
    ours:
      - key: 00000000-0000-7000-8000-0000000000a1
        label: RQ-50
";
    let base = labels_file();
    let held = format!("{base}{conflict}");
    let ours = counter(&held, "AC", 9);
    let (text, merged) = merge(&base, &ours, &held);
    assert_eq!(text.matches("field: bindings").count(), 1, "{text}");
    assert_eq!(merged.conflicts, 1);
}
