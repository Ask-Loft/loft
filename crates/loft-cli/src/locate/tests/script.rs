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
    assert!(text.contains("> some text </"), "{text}");
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
