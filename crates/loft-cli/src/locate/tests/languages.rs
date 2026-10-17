//! Rust, Python and Go tests.

use super::{canonical, located, names};

const RUST: &str = r#"
use super::*;

#[test]
fn top_level() {
    assert!(true);
}

mod inner {
    /// A helper the tests share.
    #[test]
    // why it is ignored
    #[ignore]
    fn in_a_module() {}
}

impl Shelf {
    fn in_an_impl(&self) {}
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]

    #[test]
    fn in_a_macro(count in 0..10_usize, name in "[a-z]+") {
        prop_assert!(count < 10, "{name}");
    }
}
"#;

#[test]
fn a_rust_function_is_a_test_inside_modules_and_implementations() {
    let names = names("tests/a.rs", RUST);
    for name in ["top_level", "in_a_module", "in_an_impl"] {
        assert!(names.contains(&name.to_owned()), "{names:?}");
    }
}

#[test]
fn the_attributes_before_a_rust_function_are_part_of_the_test() {
    let tests = located("tests/a.rs", RUST);
    assert!(tests.contains(&("top_level".to_owned(), 4, 7)), "{tests:?}");
    assert!(
        tests.contains(&("in_a_module".to_owned(), 11, 14)),
        "{tests:?}"
    );
    assert_eq!(
        canonical("tests/a.rs", RUST, "in_a_module"),
        "# [ test ] # [ ignore ] fn in_a_module ( ) { }"
    );
}

#[test]
fn a_function_in_a_macro_is_its_tokens_from_fn_through_its_braces() {
    let tests = located("tests/a.rs", RUST);
    assert!(
        tests.contains(&("in_a_macro".to_owned(), 24, 27)),
        "{tests:?}"
    );
    let text = canonical("tests/a.rs", RUST, "in_a_macro");
    assert!(
        text.starts_with("# [ test ] fn in_a_macro ( count in"),
        "{text}"
    );
    assert!(text.contains("\"[a-z]+\""), "{text}");
    assert!(
        text.ends_with("prop_assert ! ( count < 10 , \"{name}\" ) ; }"),
        "{text}"
    );
}

const PYTHON: &str = r#"
import pytest

def test_plain():
    assert 1 + 1 == 2

class TestShelf:
    @pytest.mark.parametrize("count", [1, 2])
    def test_in_a_class(self, count):
        if count:
            assert count
        assert True
"#;

#[test]
fn a_python_function_or_class_is_a_test_with_its_decorators() {
    let tests = located("tests/a.py", PYTHON);
    assert!(
        tests.contains(&("test_plain".to_owned(), 4, 5)),
        "{tests:?}"
    );
    assert!(
        tests.contains(&("TestShelf".to_owned(), 7, 12)),
        "{tests:?}"
    );
    assert!(
        tests.contains(&("test_in_a_class".to_owned(), 8, 12)),
        "{tests:?}"
    );
    let text = canonical("tests/a.py", PYTHON, "test_in_a_class");
    assert!(text.starts_with("@ pytest . mark . parametrize"), "{text}");
}

#[test]
fn a_python_line_s_indentation_is_part_of_the_canonical_text() {
    let text = canonical("tests/a.py", PYTHON, "test_in_a_class");
    assert!(
        text.ends_with(
            "def test_in_a_class ( self , count ) :\n        if count :\n            assert count\n        assert True"
        ),
        "{text:?}"
    );
    let dedented = PYTHON.replace(
        "            assert count\n        assert True",
        "            assert count\n            assert True",
    );
    assert_ne!(canonical("tests/a.py", &dedented, "test_in_a_class"), text);
}

const GO: &str = r#"
package shelf

func TestAdd(t *testing.T) {
	t.Run("adds a book", func(t *testing.T) {
		if got := add(1); got != 1 {
			t.Fatal(got)
		}
	})
	t.Run(`raw name`, func(t *testing.T) {})
	t.Log("not a subtest")
}

func (s *Suite) TestInASuite() {}
"#;

#[test]
fn a_go_function_and_a_subtest_are_tests() {
    let tests = located("shelf_test.go", GO);
    assert_eq!(
        tests,
        [
            ("TestAdd".to_owned(), 4, 12),
            ("adds a book".to_owned(), 5, 9),
            ("raw name".to_owned(), 10, 10),
            ("TestInASuite".to_owned(), 14, 14),
        ]
    );
}

#[test]
fn a_python_continuation_line_is_whitespace_inside_brackets_and_after_a_backslash() {
    let wrapped =
        "def test_same():\n    total = add(1,\n                2)\n    other = 1 + \\\n        2\n";
    let flat = "def test_same():\n    total = add(1, 2)\n    other = 1 + 2\n";
    assert_eq!(
        canonical("a.py", wrapped, "test_same"),
        canonical("a.py", flat, "test_same")
    );
    assert_eq!(
        canonical("a.py", flat, "test_same"),
        "def test_same ( ) :\n    total = add ( 1 , 2 )\n    other = 1 + 2"
    );
}

const QUALIFIED: &str = r#"
proptest! {
    #[test]
    async fn waits(a in 0..1) {}

    #[test]
    pub(crate) fn shared(a in 0..1) {}

    fn declared(&self) -> u32;

    #[test]
    #[ignore]
    pub unsafe extern "C" fn after_a_declaration() { body(); }
}
"#;

#[test]
fn qualifiers_in_a_macro_keep_the_attributes_and_a_declaration_is_no_test() {
    let tests = located("tests/a.rs", QUALIFIED);
    assert_eq!(
        tests,
        [
            ("waits".to_owned(), 3, 4),
            ("shared".to_owned(), 6, 7),
            ("after_a_declaration".to_owned(), 11, 13),
        ]
    );
    assert_eq!(
        canonical("tests/a.rs", QUALIFIED, "waits"),
        "# [ test ] async fn waits ( a in 0 .. 1 ) { }"
    );
    let last = canonical("tests/a.rs", QUALIFIED, "after_a_declaration");
    assert!(
        last.starts_with("# [ test ] # [ ignore ] pub unsafe extern \"C\" fn"),
        "{last}"
    );
}

#[test]
fn an_escaped_or_commented_go_subtest_name_is_read_as_written_to_run() {
    let go = "func TestNames(t *testing.T) {\n\tt.Run(\"caf\\u00e9\", nil)\n\tt.Run(\"\\101\\x42\\U0001F600\", nil)\n\tt.Run(/* why */ \"commented\", nil)\n}\n";
    let names = names("a_test.go", go);
    assert_eq!(names, ["TestNames", "café", "AB😀", "commented"]);
}

#[test]
fn go_byte_escapes_are_read_together_as_utf_8_and_a_raw_string_drops_carriage_returns() {
    let go = "func TestNames(t *testing.T) {\n\tt.Run(\"\\xc3\\xa9\", nil)\n\tt.Run(\"\\303\\251\", nil)\n\tt.Run(\"\\xf0\\x9f\\x98\\x80\", nil)\n\tt.Run(`tar\rget`, nil)\n\tt.Run(\"\\xff\", nil)\n}\n";
    // Bytes that are not UTF-8 leave the name as written.
    assert_eq!(
        names("a_test.go", go),
        ["TestNames", "é", "é", "😀", "target", "\\xff"]
    );
}

#[test]
fn a_go_subtest_run_through_parentheses_is_a_test() {
    let go = "func TestAll(t *testing.T) {\n\t(t.Run)(\"wrapped\", func(t *testing.T) {})\n}\n";
    assert_eq!(names("a_test.go", go), ["TestAll", "wrapped"]);
}

#[test]
fn a_comment_in_a_macro_does_not_end_a_test_s_attributes() {
    let read =
        |macro_body: &str| canonical("a.rs", &format!("suite! {{ {macro_body} }}\n"), "target");
    let plain = read("#[test] fn target() { assert!(true); }");
    // A comment changes nothing.
    assert_eq!(
        read("#[test] /* why */ fn target() { assert!(true); }"),
        plain
    );
    assert_eq!(
        read("#[test]\n    // why\n    fn target() { assert!(true); }"),
        plain
    );
    // An attribute or a qualifier before the comment is part of the test.
    let ignored = read("#[test] #[ignore] /* why */ fn target() { assert!(true); }");
    assert_ne!(ignored, plain);
    assert!(
        ignored.starts_with("# [ test ] # [ ignore ] fn target"),
        "{ignored}"
    );
    let crate_wide = read("#[test] pub(crate) /// why\n fn target() { assert!(true); }");
    assert!(
        crate_wide.starts_with("# [ test ] pub ( crate ) fn"),
        "{crate_wide}"
    );
}
