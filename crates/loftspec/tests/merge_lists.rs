//! The merge of one file across records and lists (F-32, F-33): a record moved to another
//! parent matched by its key, a list of values both sides reordered apart, and a merged file
//! read back.

use crate::common::text::{FEATURE, files};
use crate::merge_file::{feature, merged, three_requirements, with};
use loftspec::codec::{self, SpecPath};

/// The first Requirement's criteria, the one criterion they hold.
const CRITERIA: &str = "    criteria:
      - key: 00000000-0000-7000-8000-000000000005
        name: \"Every node record carries a `key` field.\"
        mentions:
          - 00000000-0000-7000-8000-000000000004
";

/// The last line of the second Requirement, after which its criteria go.
const SECOND_SHALL: &str = "      shall: \"write requirement c2.\"\n";

/// A criterion added after the first Requirement's criterion.
const ADDED: &str = "      - key: 00000000-0000-7000-8000-0000000000aa
        name: \"A minted key is version 7.\"
";

/// The Feature with its tags `tags`, in that order.
fn tagged(tags: &[&str]) -> String {
    let body = "body: \"Keys address nodes.\"\n";
    let list = listed("  ", tags);
    with(&feature(), &[(body, &format!("{body}tags:\n{list}"))])
}

/// The tags each side of a four-tag list gives: base, then ours and theirs, each of which moved
/// the first tag to another place.
const BASE_TAGS: [&str; 4] = ["alpha", "beta", "gamma", "delta"];
const OUR_TAGS: [&str; 4] = ["beta", "gamma", "alpha", "delta"];
const THEIR_TAGS: [&str; 4] = ["beta", "gamma", "delta", "alpha"];

/// A YAML list of `tags` at `indent`.
fn listed(indent: &str, tags: &[&str]) -> String {
    tags.iter()
        .flat_map(|tag| [indent, "- ", tag, "\n"])
        .collect()
}

#[test]
fn a_record_moved_to_another_parent_is_matched_with_its_base_record_by_its_key() {
    let base = three_requirements();
    let name = "name: \"Every node record carries a `key` field.\"\n";
    let renamed = "name: \"Every node record carries one key.\"\n";
    let bodied = format!("{name}        body: \"Minted keys too.\"\n");
    let ours = with(
        &base,
        &[
            (CRITERIA, ""),
            (SECOND_SHALL, &format!("{SECOND_SHALL}{CRITERIA}")),
            (name, renamed),
        ],
    );
    let theirs = with(
        &base,
        &[(CRITERIA, &format!("{CRITERIA}{ADDED}")), (name, &bodied)],
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    let first = "      shall: \"write a key on every node.\"\n    criteria:\n";
    assert!(
        text.contains(&format!("{first}{ADDED}")),
        "the base parent keeps only the other side's insertion: {text}"
    );
    let moved = with(CRITERIA, &[(name, &bodied.replace(name, renamed))]);
    assert!(
        text.contains(&format!("{SECOND_SHALL}{moved}")),
        "the new parent holds the record, with both sides' edits: {text}"
    );
    assert_eq!(text.matches("000000000005\n").count(), 1, "{text}");
    assert_eq!(conflicts, 0);
    let mut files = files();
    files.insert(SpecPath::new(FEATURE), text.into_bytes());
    assert!(codec::read(&files).is_ok());
}

#[test]
fn a_merged_record_that_keeps_its_base_variant_is_read_back_without_a_read_error() {
    let base = feature();
    let ours = with(
        &base,
        &[(
            "form: ubiquitous\n",
            "form: state_driven\n      while: \"a key exists\"\n",
        )],
    );
    let theirs = with(
        &base,
        &[(
            "form: ubiquitous\n",
            "form: event_driven\n      when: \"a node is written\"\n",
        )],
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert!(conflicts > 0, "{text}");
    let mut files = files();
    files.insert(SpecPath::new(FEATURE), text.into_bytes());
    let read = codec::read(&files);
    assert!(read.is_ok(), "{:?}", read.err());
}

#[test]
fn a_value_both_sides_move_apart_gives_one_conflict_holding_each_side_s_whole_list() {
    let (text, conflicts) = merged(
        &tagged(&BASE_TAGS),
        &tagged(&OUR_TAGS),
        &tagged(&THEIR_TAGS),
    );
    assert_eq!(conflicts, 1, "{text}");
    let conflict = format!(
        "conflicts:\n  - field: tags\n    base:\n{}    ours:\n{}    theirs:\n{}",
        listed("      ", &BASE_TAGS),
        listed("      ", &OUR_TAGS),
        listed("      ", &THEIR_TAGS),
    );
    assert!(text.ends_with(&conflict), "{text}");
}

#[test]
fn a_list_of_values_both_sides_reorder_apart_keeps_its_base_order() {
    let (text, _) = merged(
        &tagged(&BASE_TAGS),
        &tagged(&OUR_TAGS),
        &tagged(&THEIR_TAGS),
    );
    let tags = format!(
        "body: \"Keys address nodes.\"\ntags:\n{}realizes:\n",
        listed("  ", &BASE_TAGS)
    );
    assert!(text.contains(&tags), "{text}");
}
