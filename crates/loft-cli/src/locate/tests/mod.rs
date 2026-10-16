//! The test locator over small files of each language.

mod canonical;
mod languages;
mod script;

use super::locate;

/// Each test `text`, the file at `path`, holds: its name and its first and last lines.
fn located(path: &str, text: &str) -> Vec<(String, usize, usize)> {
    locate(path, text)
        .unwrap_or_else(|| panic!("no grammar parses {path}"))
        .into_iter()
        .map(|test| (test.name, test.lines.first, test.lines.last))
        .collect()
}

/// The names of the tests `text`, the file at `path`, holds.
fn names(path: &str, text: &str) -> Vec<String> {
    located(path, text)
        .into_iter()
        .map(|(name, _, _)| name)
        .collect()
}

/// The canonical text of the one test named `name` in `text`, the file at `path`.
fn canonical(path: &str, text: &str, name: &str) -> String {
    let tests = locate(path, text).unwrap();
    let named: Vec<_> = tests.into_iter().filter(|test| test.name == name).collect();
    assert_eq!(named.len(), 1, "{named:?}");
    named[0].canonical.clone()
}

#[test]
fn each_extension_chooses_its_grammar_and_any_other_is_not_parsed() {
    for path in [
        "a.rs", "a.ts", "a.mts", "a.cts", "a.tsx", "a.js", "a.mjs", "a.cjs", "a.jsx", "a.py",
        "a.go",
    ] {
        assert!(locate(path, "").is_some(), "{path}");
    }
    for path in ["a.rb", "Makefile", "a.rs.txt", "a"] {
        assert_eq!(locate(path, "fn a() {}"), None, "{path}");
    }
}
