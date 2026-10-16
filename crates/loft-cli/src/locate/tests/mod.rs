//! The test locator over small files of each language.

mod canonical;
mod languages;
mod script;

use super::locate;

/// Each test `text`, the file at `path`, holds: its name and its first and last lines.
fn located(path: &str, text: &str) -> Vec<(String, usize, usize)> {
    locate(path, text)
        .unwrap_or_else(|| panic!("no grammar parses {path}"))
        .tests
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
    locate(path, text).unwrap().errors.is_empty()
}

/// Each part of `text`, the file at `path`, that the grammar does not parse, as the reconciler
/// gives it.
fn unparsed(path: &str, text: &str) -> Vec<String> {
    let errors = locate(path, text).unwrap().errors;
    errors.iter().map(ToString::to_string).collect()
}

/// The canonical text of the one test named `name` in `text`, the file at `path`.
fn canonical(path: &str, text: &str, name: &str) -> String {
    let tests = locate(path, text).unwrap().tests;
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

#[test]
fn a_part_that_does_not_parse_is_given_by_line_column_and_text_or_the_missing_token() {
    // Valid Python that the grammar does not parse: a line inside brackets less indented than
    // the line it continues.
    let python = "def target():\n    assert (1 ==\n1)\ntarget()\n";
    assert_eq!(
        unparsed("a.py", python),
        ["2:5, at `assert (1 ==`", "3:1, at `1)`"]
    );
    assert_eq!(
        unparsed("a.rs", "fn a() { let x = 1 }\n"),
        ["1:19, where `;` is missing"]
    );
    assert_eq!(
        unparsed("a.go", "package p\nfunc TestA(t *testing.T) { x := f(1 }\n"),
        ["2:36, where `)` is missing"]
    );
    // A column counts characters, not bytes.
    assert_eq!(
        unparsed("a.ts", "// é\ntest(\"é\", () => { f(1; })\n"),
        ["2:19, at `f(1`"]
    );
    assert_eq!(
        unparsed("a.ts", "test(\"x\", () => {})\n"),
        Vec::<String>::new()
    );
}
