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
    let added = "#[test] fn same() { let a = vec![1, 2,]; }\n";
    assert_ne!(
        canonical("a.rs", tight, "same"),
        canonical("a.rs", added, "same")
    );
}

/// One test in each grammar, and the SHA-256 of its canonical text. A new version of a grammar
/// that changes one of them would report every such test as changed after it was recorded.
const GOLDEN: [(&str, &str, &str, &str); 6] = [
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
