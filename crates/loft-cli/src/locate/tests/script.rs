//! JavaScript and TypeScript tests.

use super::{canonical, located, names};

const SPEC: &str = r#"
import { describe, it, test } from "vitest";

describe("the shelf", () => {
  it("adds a book", () => {
    expect(add(1)).toBe(1);
  });
  it.only("lists \"quoted\" titles", () => {});
  it.skip.each(["a", "b"])("refuses %s", (value) => {});
  test.concurrent.each`
    a    | b
    ${1} | ${2}
  `("sums $a and $b", ({ a, b }) => {});
  it.skipIf(process.env.CI)(`a template title`, () => {});
  it(`a ${"substituted"} title`, () => {});
  context("a context", () => {});
  specify("a specification", () => {});
  bench("a benchmark", () => {});
});

check("a check is not a test", ok);
describe("same name", () => {
  it("same name", () => {});
});
"#;

#[test]
fn a_call_that_registers_a_test_with_a_string_name_is_a_test() {
    let names = names("test/shelf.spec.ts", SPEC);
    for name in [
        "the shelf",
        "adds a book",
        "a context",
        "a specification",
        "a benchmark",
    ] {
        assert!(names.contains(&name.to_owned()), "{name}: {names:?}");
    }
    assert!(!names.contains(&"a check is not a test".to_owned()));
}

#[test]
fn a_modifier_or_a_called_property_registers_as_the_plain_call_does() {
    let names = names("test/shelf.spec.js", SPEC);
    for name in ["lists \"quoted\" titles", "refuses %s", "sums $a and $b"] {
        assert!(names.contains(&name.to_owned()), "{name}: {names:?}");
    }
}

#[test]
fn a_template_literal_names_a_test_only_without_a_substitution() {
    let names = names("test/shelf.spec.mjs", SPEC);
    assert!(names.contains(&"a template title".to_owned()), "{names:?}");
    assert!(!names.iter().any(|name| name.contains("substituted")));
}

#[test]
fn of_nested_calls_with_one_name_the_outermost_is_the_test() {
    let tests = located("test/shelf.spec.ts", SPEC);
    let same: Vec<_> = tests
        .iter()
        .filter(|(name, _, _)| name == "same name")
        .collect();
    assert_eq!(same, [&("same name".to_owned(), 22, 24)]);
}

#[test]
fn a_tsx_file_holds_tests_beside_jsx() {
    let spec = "test('renders the pane', () => {\n  render(<Pane title=\"Loft\">  some\n   text </Pane>);\n});\n";
    assert_eq!(
        located("tests/pane.test.tsx", spec),
        [("renders the pane".to_owned(), 1, 4)]
    );
    let text = canonical("tests/pane.test.tsx", spec, "renders the pane");
    // JSX keeps the spaces that open the text's first line and trims the rest where it breaks.
    assert!(text.contains(">   some text  </"), "{text}");
}

#[test]
fn jsx_text_is_read_as_jsx_reads_it() {
    let read = |body: &str| {
        let spec = format!("test(\"t\", () => {{\n  check(<p>{body}</p>);\n}});\n");
        canonical("a.test.jsx", &spec, "t")
    };
    // Spaces within a line are text.
    assert_ne!(read("hello world"), read("hello  world"));
    // Where the text breaks across lines, the indentation around each break is not.
    assert_eq!(
        read("\n    hello\n    world\n  "),
        read("\n  hello\n        world\n")
    );
    assert_eq!(read("\n    hello\n    world\n  "), read("hello world"));
    assert_eq!(
        canonical(
            "a.test.tsx",
            "test(\"t\", () => <p>hello  world</p>);\n",
            "t"
        ),
        "test ( \"t\" , ( ) => < p > hello  world </ p > )"
    );
}

#[test]
fn a_test_located_twice_has_one_location_for_each() {
    let spec = "it(\"refuses %s\", () => {});\nit.each([1])(\"refuses %s\", (n) => {});\n";
    assert_eq!(
        located("a.test.js", spec),
        [
            ("refuses %s".to_owned(), 1, 1),
            ("refuses %s".to_owned(), 2, 2)
        ]
    );
}

#[test]
fn a_plain_test_and_every_modifier_register_a_test() {
    let spec = "test(\"plain\", () => {});\nit.todo(\"todo\");\nit.sequential(\"sequential\", () => {});\nit.fails(\"fails\", () => {});\nit.runIf(true)(\"run if\", () => {});\ndescribe.skipIf(false)(\"skip if\", () => {});\n";
    assert_eq!(
        names("a.test.js", spec),
        ["plain", "todo", "sequential", "fails", "run if", "skip if"]
    );
}

#[test]
fn an_escape_in_a_name_is_read_as_its_character_and_a_comment_is_no_argument() {
    let spec = "it(\"caf\\u00e9\", () => {});\nit('\\x41\\u{1F600}\\n', () => {});\nit(/* why */ \"commented\", () => {});\n";
    assert_eq!(names("a.test.ts", spec), ["café", "A😀\n", "commented"]);
}

#[test]
fn a_tsx_file_is_parsed_as_typescript_with_jsx() {
    let spec = "test(\"typed\", () => {\n  const pane = render<Props>(<Pane title={\"x\" as string} />);\n  const size: number = 1;\n});\n";
    assert_eq!(names("pane.test.tsx", spec), ["typed"]);
    assert!(super::parses_cleanly("pane.test.tsx", spec));
    // The JavaScript grammar cannot read the type annotations.
    assert!(!super::parses_cleanly("pane.test.jsx", spec));
    let text = canonical("pane.test.tsx", spec, "typed");
    assert!(text.contains("render < Props > ( < Pane"), "{text}");
}

#[test]
fn a_surrogate_pair_is_one_character_and_a_backslash_before_any_line_break_continues() {
    let spec = "it(\"\\ud83d\\ude00\", () => {});\nit(\"lone \\ud83d\", () => {});\nit(\"\\a\\U\", () => {});\n";
    assert_eq!(names("a.test.js", spec), ["😀", "lone \\ud83d", "aU"]);
    for line_break in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        let spec = format!("it(\"tar\\{line_break}get\", () => {{}});\n");
        assert_eq!(names("a.test.js", &spec), ["target"], "{line_break:?}");
    }
}

#[test]
fn a_registering_function_in_parentheses_registers_a_test() {
    let spec = "(test)(\"wrapped\", () => {});\n((it.only))(\"twice wrapped\", () => {});\n(describe).each([1])(\"called\", () => {});\n";
    assert_eq!(
        names("a.test.ts", spec),
        ["wrapped", "twice wrapped", "called"]
    );
}
