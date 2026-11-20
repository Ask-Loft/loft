//! The help of `loft remote set`'s address: the form of a document's address on a Loft server,
//! with the key of the manifest in the working directory where one reads.

use crate::common::{Scratch, tree};

/// The help `loft remote set --help` prints in `scratch`.
fn help(scratch: &Scratch) -> String {
    let output = scratch.loft(&["remote", "set", "--help"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let printed = String::from_utf8_lossy(&output.stdout);
    printed.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn beside_a_manifest_the_help_gives_the_form_with_its_document_key() {
    let scratch = Scratch::with_reading_list();
    let key = loftspec::codec::read(&tree(&scratch.0))
        .unwrap()
        .id
        .key()
        .to_string();
    let help = help(&scratch);
    let form = format!("https://<host>/loft/t/<tenant-slug>/api/v2/documents/{key}/remote");
    assert!(help.contains(&form), "{help}");
    assert!(!help.contains("<documentKey>"), "{help}");
}

#[test]
fn without_a_manifest_or_with_one_that_does_not_read_the_help_gives_the_placeholder() {
    let unreadable = Scratch::with_reading_list();
    std::fs::write(unreadable.file("loftspec.yaml"), "documentKey: [a, b]\n").unwrap();
    for scratch in [Scratch::empty(), unreadable] {
        let help = help(&scratch);
        assert!(
            help.contains("/api/v2/documents/<documentKey>/remote"),
            "{help}"
        );
        assert!(
            help.contains("<documentKey> is the `documentKey` of spec/loftspec.yaml"),
            "{help}"
        );
    }
}
