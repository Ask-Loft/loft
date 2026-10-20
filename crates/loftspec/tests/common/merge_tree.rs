//! Three versions of the small document's tree, made as documents and edited as documents, and
//! their merge as whole trees.

use std::fmt::Write as _;

use loftspec::codec::{self, Files, SpecPath};
use loftspec::document::Document;
use loftspec::ids::RequirementId;
use loftspec::merge::{MergedTree, merge_file, merge_tree};
use loftspec::tree::{Feature, Requirement};

use super::text::linted;
use super::{criterion, feature, key, requirement, small_document};

/// The Feature file of the third Feature, `Aliases`.
pub const ALIASES_FEATURE: &str = "products/example-product/identity/aliases.yaml";

/// The Feature file of the second Feature, `Labels`.
pub const LABELS_FEATURE: &str = "products/example-product/identity/labels.yaml";

/// The small document with a second Feature, `Labels` keyed 40, after `Keys` in its group,
/// holding the Requirement keyed 41 with the criterion keyed 42.
pub fn two_features() -> Document {
    let mut document = small_document();
    let labels = feature(
        key(40),
        "Labels",
        vec![requirement(
            key(41),
            "The writer shall write a label.",
            vec![criterion(key(42), "A label is written.")],
        )],
    );
    document.products[0].feature_groups[0].features.push(labels);
    document
}

/// [`two_features`] with a third Feature, `Aliases` keyed 46, holding no Requirement.
pub fn three_features() -> Document {
    let mut document = two_features();
    features(&mut document).push(feature(key(46), "Aliases", Vec::new()));
    document
}

/// The Features of the document's one group.
pub fn features(document: &mut Document) -> &mut Vec<Feature> {
    &mut document.products[0].feature_groups[0].features
}

/// The Requirements of the group's Feature at `at`.
pub fn requirements(document: &mut Document, at: usize) -> &mut Vec<Requirement> {
    &mut features(document)[at].requirements
}

/// `base` with the Requirement keyed 4 moved from `Keys` to the end of `Labels`.
pub fn moved(base: &Document) -> Document {
    let mut ours = base.clone();
    let moving = requirements(&mut ours, 0).remove(0);
    requirements(&mut ours, 1).push(moving);
    ours
}

/// `base` with the body of the Requirement keyed 4 set to `body`, wherever it is.
pub fn edited(base: &Document, body: &str) -> Document {
    let mut theirs = base.clone();
    requirement_4(&mut theirs).details.body = Some(body.to_owned());
    theirs
}

/// The Requirement keyed 4, in whichever Feature of the group holds it.
pub fn requirement_4(document: &mut Document) -> &mut Requirement {
    let id = Some(RequirementId::new(key(4)));
    features(document)
        .iter_mut()
        .flat_map(|feature| &mut feature.requirements)
        .find(|requirement| requirement.id == id)
        .expect("the Requirement keyed 4")
}

/// Merges the three documents' trees, which must merge.
pub fn merged(base: &Document, ours: &Document, theirs: &Document) -> MergedTree {
    merged_files(
        &codec::write(base),
        &codec::write(ours),
        &codec::write(theirs),
    )
}

/// Merges three trees, which must merge.
pub fn merged_files(base: &Files, ours: &Files, theirs: &Files) -> MergedTree {
    merge_tree(base, ours, theirs).expect("the trees merge")
}

/// The merged tree, read; it must read.
pub fn read(merged: &MergedTree) -> Document {
    codec::read(&merged.files).unwrap_or_else(|errors| panic!("{errors:?}\n{}", dump(merged)))
}

/// The text of the merged tree's file at `at`.
pub fn file(merged: &MergedTree, at: &str) -> String {
    let bytes = merged.files.get(&SpecPath::new(at));
    String::from_utf8(
        bytes
            .unwrap_or_else(|| panic!("{at} in\n{}", dump(merged)))
            .clone(),
    )
    .unwrap()
}

/// Every file of the merged tree, for a failure message.
pub fn dump(merged: &MergedTree) -> String {
    let mut text = String::new();
    for (path, bytes) in &merged.files {
        writeln!(text, "--- {path}\n{}", String::from_utf8_lossy(bytes)).unwrap();
    }
    text
}

/// How many records of the merged tree carry the key numbered `n`, apart from its binding.
pub fn carried(merged: &MergedTree, n: u32) -> usize {
    let line = format!("key: {}\n", key(n));
    merged
        .files
        .iter()
        .filter(|(path, _)| path.as_str() != codec::LABELS)
        .map(|(_, bytes)| String::from_utf8_lossy(bytes).matches(&line).count())
        .sum()
}

/// The merge driver's file for `path` from the three trees, which must merge; an absent base
/// file is an empty one, as git gives.
pub fn driver(path: &str, base: &Files, ours: &Files, theirs: &Files) -> Vec<u8> {
    let path = SpecPath::new(path);
    let empty = Vec::new();
    let [base, ours, theirs] = [base, ours, theirs].map(|files| files.get(&path).unwrap_or(&empty));
    merge_file(&path, base, ours, theirs)
        .expect("the file merges")
        .bytes
}

/// The tree of `document` with `spec/lint.yaml` holding `text`.
pub fn with_lint(document: &Document, text: &str) -> Files {
    linted(&codec::write(document), Some(text))
}
