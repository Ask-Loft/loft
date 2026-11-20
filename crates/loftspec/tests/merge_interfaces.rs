//! Interfaces in a three-way merge: one Interface file changed on both sides merges as any record
//! file does, its separate changes kept and a clash left as a conflict record in that file.

use crate::common::interfaces::with_interface;
use crate::common::merge_tree::{dump, file, merged, read};
use loftspec::document::Document;

const INTERFACE_FILE: &str = "interfaces/document-lookup.yaml";

fn with_body(body: &str) -> Document {
    let mut document = with_interface();
    document.interfaces[0].details.body = Some(body.to_owned());
    document
}

#[test]
fn separate_changes_to_one_interface_file_on_both_sides_are_both_kept() {
    let base = with_interface();
    let ours = with_body("Changed on our side.");
    let mut theirs = with_interface();
    theirs.interfaces[0].requirements[0].criteria[0].name = "Changed on theirs.".to_owned();
    let merged = merged(&base, &ours, &theirs);
    assert_eq!(merged.conflicts, 0, "{}", dump(&merged));
    let document = read(&merged);
    let interface = &document.interfaces[0];
    assert_eq!(
        interface.details.body.as_deref(),
        Some("Changed on our side.")
    );
    assert_eq!(
        interface.requirements[0].criteria[0].name,
        "Changed on theirs."
    );
}

#[test]
fn a_clash_in_one_interface_file_is_a_conflict_record_in_that_file() {
    let merged = merged(
        &with_interface(),
        &with_body("Ours."),
        &with_body("Theirs."),
    );
    assert_eq!(merged.conflicts, 1, "{}", dump(&merged));
    let text = file(&merged, INTERFACE_FILE);
    assert!(text.contains("conflicts:"), "{text}");
    assert!(text.contains("Ours.") && text.contains("Theirs."), "{text}");
}
