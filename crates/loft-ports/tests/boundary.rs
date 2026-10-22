//! `loft-ports` holds what both stores share and nothing that knows a document or Yard Log: it
//! depends on no crate of the Format and on no store.

use std::fs;
use std::path::Path;

use crate::common::{dependencies, manifest};

/// The crates of Loft that the ports never depend on.
const ABOVE: [&str; 5] = [
    "loftspec",
    "loftspec-store",
    "yardlog",
    "yardlog-store",
    "loft-cli",
];

#[test]
fn the_ports_depend_on_no_crate_of_the_format_and_on_no_store() {
    let listed = dependencies(&manifest("loft-ports"));
    assert!(listed.contains(&"thiserror".to_owned()), "{listed:?}");
    for krate in ABOVE {
        assert!(
            !listed.iter().any(|name| name == krate),
            "loft-ports depends on {krate}"
        );
    }
}

#[test]
fn no_source_of_the_ports_names_an_item_of_the_format_or_a_store() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut seen = 0;
    for entry in fs::read_dir(src).unwrap() {
        let text = fs::read_to_string(entry.unwrap().path()).unwrap();
        seen += 1;
        for krate in ABOVE {
            let path = format!("{}::", krate.replace('-', "_"));
            assert!(!text.contains(&path), "a source names {path}");
        }
    }
    assert!(seen > 5, "the scan found {seen} files");
}
