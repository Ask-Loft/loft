//! The test locator is the only part of `loft` that parses a programming language: no source of
//! a crate outside `crates/loft-cli/src/locate/` names tree-sitter, and no manifest but `loft-cli`'s takes
//! it, so the libraries stay free of its C code.

use std::fs;

use crate::common::{shown, sources, workspace};

#[test]
fn only_the_test_locator_parses_a_programming_language() {
    let users: Vec<String> = sources(&workspace().join("crates"))
        .into_iter()
        .map(|path| (shown(&path), path))
        .filter(|(shown, _)| shown.contains("/src/"))
        .filter(|(_, path)| fs::read_to_string(path).unwrap().contains("tree_sitter"))
        .map(|(shown, _)| shown)
        .filter(|shown| !shown.starts_with("crates/loft-cli/src/locate/"))
        .collect();
    assert_eq!(users, Vec::<String>::new());
    for library in ["loftspec", "loftspec-store", "software-plant"] {
        let manifest = workspace().join("crates").join(library).join("Cargo.toml");
        let manifest = fs::read_to_string(manifest).unwrap();
        assert!(!manifest.contains("tree-sitter"), "{library}");
    }
}
