//! The small document's Feature file and the edits the merge tests make to it.

use loftspec::codec::{self, SpecPath};
use loftspec::merge::{Merged, merge_file};

use super::text::{FEATURE, files, text};

/// The criterion's name.
pub const CRITERION: &str = "name: \"Every node record carries a `key` field.\"";

/// The criterion's name as one side edits it.
pub const EDITED: &str = "name: \"Every node record carries one key.\"";

/// The first Requirement's criteria, the one criterion they hold.
pub const CRITERIA: &str = "    criteria:
      - key: 00000000-0000-7000-8000-000000000005
        name: \"Every node record carries a `key` field.\"
        mentions:
          - 00000000-0000-7000-8000-000000000004
";

pub fn feature() -> String {
    text(&files(), FEATURE)
}

/// `text` with each `(from, to)` replaced once.
pub fn with(text: &str, edits: &[(&str, &str)]) -> String {
    let mut text = text.to_owned();
    for (from, to) in edits {
        assert!(text.contains(from), "{from:?} in\n{text}");
        text = text.replacen(from, to, 1);
    }
    text
}

/// Merges the file at `path` with `merge_file`, which must merge.
pub fn merge_at(path: &str, base: &str, ours: &str, theirs: &str) -> Merged {
    let [base, ours, theirs] = [base, ours, theirs].map(str::as_bytes);
    merge_file(&SpecPath::new(path), base, ours, theirs).expect("the file merges")
}

/// Merges the file at `path` and gives the result as text.
pub fn merged_at(path: &str, base: &str, ours: &str, theirs: &str) -> (String, usize) {
    let merged = merge_at(path, base, ours, theirs);
    (String::from_utf8(merged.bytes).unwrap(), merged.conflicts)
}

/// Merges the Feature file and gives the result as text.
pub fn merged(base: &str, ours: &str, theirs: &str) -> (String, usize) {
    merged_at(FEATURE, base, ours, theirs)
}

/// The small document's tree with the file at `at` replaced by `text`, read back.
pub fn read_back(at: &str, text: &str) {
    let mut files = files();
    files.insert(SpecPath::new(at), text.as_bytes().to_vec());
    let read = codec::read(&files);
    assert!(read.is_ok(), "{:?}\n{text}", read.err());
}

/// A Requirement of the Feature keyed `…0000000000{n}`, whose statement ends with `shall(n)`.
pub fn requirement(n: &str) -> String {
    format!(
        "  - key: 00000000-0000-7000-8000-0000000000{n}
    statement:
      form: ubiquitous
      system: \"The writer\"
{}",
        shall(n)
    )
}

/// The last line of the statement of the Requirement keyed `n`, after which its criteria go.
pub fn shall(n: &str) -> String {
    format!("      shall: \"write requirement {n}.\"\n")
}

/// `base` with the criterion moved from the first Requirement to the one keyed `n`.
pub fn moved_to(base: &str, n: &str) -> String {
    let after = shall(n);
    with(
        base,
        &[(CRITERIA, ""), (&after, &format!("{after}{CRITERIA}"))],
    )
}

/// The Feature with a second and a third Requirement after the first.
pub fn three_requirements() -> String {
    with(
        &feature(),
        &[(
            "qualityAttributes:\n",
            &format!(
                "{}{}qualityAttributes:\n",
                requirement("c2"),
                requirement("c3")
            ),
        )],
    )
}
