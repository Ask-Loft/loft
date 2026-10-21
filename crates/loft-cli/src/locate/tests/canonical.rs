//! The canonical text: tokens joined by one space, without comments, each literal whole, and
//! the same text whatever the whitespace between tokens.

use super::canonical;

#[test]
fn the_canonical_text_joins_the_tokens_with_one_space_without_comments() {
    let text =
        "#[test]\nfn joined() {\n    // a comment\n    let total = 1 + 2; /* another */\n}\n";
    assert_eq!(
        canonical("a.rs", text, "joined"),
        "# [ test ] fn joined ( ) { let total = 1 + 2 ; }"
    );
}

#[test]
fn a_literal_is_one_token_written_as_in_the_file() {
    let text = "#[test]\nfn spaced() {\n    let words = \"two  spaces\";\n    let c = ' ';\n}\n";
    assert_eq!(
        canonical("a.rs", text, "spaced"),
        "# [ test ] fn spaced ( ) { let words = \"two  spaces\" ; let c = ' ' ; }"
    );
    let script = "it(\"a\", () => {\n  const t = `x  ${y}  z`;\n  const r = /a  b/g;\n});\n";
    assert_eq!(
        canonical("a.test.ts", script, "a"),
        "it ( \"a\" , ( ) => { const t = `x  ${y}  z` ; const r = /a  b/g ; } )"
    );
}

#[test]
fn whitespace_between_tokens_leaves_the_canonical_text_unchanged() {
    let tight = "#[test] fn same() { let a = vec![1, 2]; }\n";
    let loose = "#[test]\nfn   same()\n{\n\n    let a =\n        vec![1,   2];\n}\n";
    assert_eq!(
        canonical("a.rs", tight, "same"),
        canonical("a.rs", loose, "same")
    );
    let changed = "#[test] fn same() { let a = vec![1, 3]; }\n";
    assert_ne!(
        canonical("a.rs", tight, "same"),
        canonical("a.rs", changed, "same")
    );
}

#[test]
fn a_comma_before_a_closing_bracket_is_not_a_token_except_in_python() {
    let pairs = [
        (
            "a.rs",
            "fn same() { f(1, 2); let v = [1, 2]; }\n",
            "fn same() {\n    f(\n        1,\n        2,\n    );\n    let v = [1, 2,];\n}\n",
        ),
        (
            "a.test.ts",
            "it(\"same\", () => { f({ a: 1 }, [2]) })\n",
            "it(\"same\", () => { f({ a: 1, }, [2,],) },)\n",
        ),
        (
            "a_test.go",
            "func same() { f(T{a: 1}) }\n",
            "func same() {\n\tf(T{\n\t\ta: 1,\n\t},)\n}\n",
        ),
    ];
    for (path, without, with) in pairs {
        assert_eq!(
            canonical(path, without, "same"),
            canonical(path, with, "same"),
            "{path}"
        );
    }
    let tuple = "def same():\n    return (1,)\n";
    let number = "def same():\n    return (1)\n";
    assert_ne!(
        canonical("a.py", tuple, "same"),
        canonical("a.py", number, "same")
    );
}

#[test]
fn a_line_break_written_as_cr_lf_is_read_as_lf() {
    let pairs = [
        ("a.rs", "#[test]\nfn same() {\n    let s = r\"x\ny\";\n}\n"),
        (
            "a.test.js",
            "it(\"same\", () => {\n  const t = `x\ny`;\n});\n",
        ),
        (
            "a.py",
            "def same():\n    s = \"\"\"x\ny\"\"\"\n    z = 1 + \\\n        2\n",
        ),
        ("a_test.go", "func same() {\n\ts := `x\ny`\n}\n"),
    ];
    for (path, unix) in pairs {
        let windows = unix.replace('\n', "\r\n");
        assert_eq!(
            canonical(path, unix, "same"),
            canonical(path, &windows, "same"),
            "{path}"
        );
    }
    let name = "it('a \\\r\nb', () => {});\r\n";
    assert_eq!(super::names("a.test.js", name), ["a b"]);
}

#[test]
fn locating_takes_time_in_proportion_to_the_file() {
    use std::fmt::Write as _;
    use std::time::Instant;
    let file = |tests: usize| -> String {
        let mut text = String::new();
        for at in 0..tests {
            writeln!(text, "def test_{at}(value):\n    assert value == {at}\n").unwrap();
        }
        text
    };
    let fastest = |tests: usize| {
        let text = file(tests);
        (0..3)
            .map(|_| {
                let started = Instant::now();
                assert_eq!(
                    super::super::locate("a.py", &text).unwrap().tests.len(),
                    tests
                );
                started.elapsed()
            })
            .min()
            .unwrap()
    };
    let (small, large) = (fastest(1_000), fastest(4_000));
    // Four times the tests: about four times the time where locating is linear, sixteen times
    // where each test walked the whole file.
    assert!(
        large < small * 10,
        "1,000 tests: {small:?}; 4,000 tests: {large:?}"
    );
}

/// One test in each grammar, and the SHA-256 of its canonical text. A new version of a grammar
/// that changes one of them would report every such test as changed after it was recorded.
const GOLDEN: [(&str, &str, &str, &str); 7] = [
    (
        "a.rs",
        "#[test]\nfn adds() {\n    assert_eq!(add(1, 2), 3, \"sum\");\n}\n",
        "adds",
        "f9adb7ca4feae2eea5da008c9aae9c0a61a1fba1487d9066a74e8993739ad36b",
    ),
    (
        "a.test.ts",
        "it(\"adds\", () => {\n  expect(add(1, 2) as number).toBe(3);\n});\n",
        "adds",
        "d788e65e393b2d7938f50b47d0408931d6bae48e41c013d9f3c4acfdb3e1b691",
    ),
    (
        "a.test.tsx",
        "test(\"adds\", () => {\n  render(<Sum a={1} b={2} />);\n});\n",
        "adds",
        "094e8a291b8d49b602159f86934d208c7715319b4a5873598ae9f7804f63561a",
    ),
    (
        "a.test.mjs",
        "test(`adds`, async () => {\n  assert.equal(await add(1, 2), 3);\n});\n",
        "adds",
        "707374835747610dea4a5d4c95a0ad34917ebdeee680128b2bc2a29d88d7d2c3",
    ),
    (
        "test_a.py",
        "@mark.slow\ndef test_adds():\n    assert add(1, 2) == 3\n",
        "test_adds",
        "a49dfd23bfe96e43c42d39c4b22a57ad58a54d539929e263dee3432bb8ec825d",
    ),
    (
        "a_test.go",
        "func TestAdds(t *testing.T) {\n\tif add(1, 2) != 3 {\n\t\tt.Fatal(\"sum\")\n\t}\n}\n",
        "TestAdds",
        "8ea84b8efd14f6205844ed363e0c0bc97da063b267aa7ee0e29f6acea8556de3",
    ),
    (
        "AddsTest.java",
        "class AddsTest {\n    @Test\n    void adds() {\n        assertEquals(3, add(1, 2), \"sum\");\n    }\n}\n",
        "adds",
        "81d26749628cf3c6d5023da98ea64cf56689b95a5b58b927bf26636a0734c61c",
    ),
];

#[test]
fn the_pinned_grammars_give_each_known_test_its_known_digest() {
    use sha2::{Digest as _, Sha256};
    for (path, text, name, digest) in GOLDEN {
        let canonical = canonical(path, text, name);
        let actual = format!("{:x}", Sha256::digest(&canonical));
        assert_eq!(actual, digest, "{path}: {canonical}");
    }
}
