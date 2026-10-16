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

/// Whether the grammar `path` chooses parses `text` without an error.
fn parses_cleanly(path: &str, text: &str) -> bool {
    let grammar = super::Grammar::of(path).unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&grammar.language()).unwrap();
    !parser.parse(text, None).unwrap().root_node().has_error()
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
    use super::Grammar;
    let chosen = [
        ("a.rs", Grammar::Rust),
        ("a.ts", Grammar::TypeScript),
        ("a.mts", Grammar::TypeScript),
        ("a.cts", Grammar::TypeScript),
        ("a.tsx", Grammar::Tsx),
        ("a.js", Grammar::JavaScript),
        ("a.mjs", Grammar::JavaScript),
        ("a.cjs", Grammar::JavaScript),
        ("a.jsx", Grammar::JavaScript),
        ("a.py", Grammar::Python),
        ("a.go", Grammar::Go),
    ];
    for (path, grammar) in chosen {
        assert_eq!(Grammar::of(path), Some(grammar), "{path}");
    }
    for path in ["a.rb", "Makefile", "a.rs.txt", "a"] {
        assert_eq!(locate(path, "fn a() {}"), None, "{path}");
    }
}

#[test]
fn a_test_is_located_in_a_file_of_each_extension() {
    let typed = "it(\"typed\", (value: number): void => {});\n";
    let jsx = "it(\"rendered\", () => render(<Pane />));\n";
    let plain = "it(\"plain\", () => {});\n";
    for (path, text, name) in [
        ("a.test.mts", typed, "typed"),
        ("a.test.cts", typed, "typed"),
        ("a.test.cjs", plain, "plain"),
        ("a.test.jsx", jsx, "rendered"),
    ] {
        assert_eq!(names(path, text), [name], "{path}");
        assert!(parses_cleanly(path, text), "{path}");
    }
}
