//! The small document's Feature file and the edits the merge tests make to it.

use loftspec::codec::schema::Kind;
use loftspec::codec::{self, Role, SpecPath};
use loftspec::merge::{MergeError, Merged, merge_file, merge_file_legacy};

use super::edit_tree::Top;
use super::merge_keys::Versions;
use super::merge_known::known_difference;
use super::text::{FEATURE, files, text};

/// The criterion's name.
pub const CRITERION: &str = "name: \"Every node record carries a `key` field.\"";

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

/// Merges the file at `path` with `merge_file`, which must merge, and asserts that the merge by
/// key and the legacy merge give the same unless the case is one of their known differences.
pub fn merge_at(path: &str, base: &str, ours: &str, theirs: &str) -> Merged {
    let inputs = [base, ours, theirs].map(str::as_bytes);
    let spec_path = SpecPath::new(path);
    let legacy = merge_file_legacy(&spec_path, inputs[0], inputs[1], inputs[2]);
    agrees_by_key(path, inputs, &legacy);
    merge_file(&spec_path, inputs[0], inputs[1], inputs[2]).expect("the file merges")
}

/// Asserts that `merge_file` gives `expected`, what `merge_file_legacy` gave, unless the
/// case is one of the merge by key's known differences.
pub fn agrees_by_key(path: &str, inputs: [&[u8]; 3], expected: &Result<Merged, MergeError>) {
    let spec_path = SpecPath::new(path);
    let top = Role::of(&spec_path).and_then(|role| top(&role));
    if top.is_some_and(|top| known_difference(&Versions::of(top, inputs)).is_some()) {
        return;
    }
    let by_key = merge_file(&spec_path, inputs[0], inputs[1], inputs[2]);
    let text = |merged: &Result<Merged, MergeError>| match merged {
        Ok(merged) => format!(
            "{} conflicts:\n{}",
            merged.conflicts,
            String::from_utf8_lossy(&merged.bytes)
        ),
        Err(error) => format!("{error:?}"),
    };
    assert!(
        by_key == *expected,
        "the merge by key gives {}\nwhere the legacy merge gives {}",
        text(&by_key),
        text(expected)
    );
}

/// What a file holds at its top, for the files whose keyed records the merge by key merges.
fn top(role: &Role) -> Option<Top> {
    Some(match role {
        Role::Tags | Role::Categories => Top::Registry,
        Role::Product { .. } => Top::Record(Kind::Product),
        Role::Feature { .. } => Top::Record(Kind::Feature),
        Role::Component { .. } => Top::Record(Kind::RootComponent),
        Role::Decision { .. } => Top::Record(Kind::Decision),
        Role::Manifest | Role::Labels | Role::Lint => return None,
    })
}

/// Merges the Feature file and gives the result as text.
pub fn merged(base: &str, ours: &str, theirs: &str) -> (String, usize) {
    let merged = merge_at(FEATURE, base, ours, theirs);
    (String::from_utf8(merged.bytes).unwrap(), merged.conflicts)
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
