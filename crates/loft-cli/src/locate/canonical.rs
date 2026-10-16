//! A test's canonical text: its tokens in the order of the file, joined by one space, without
//! comments. A string, character, raw string, regular expression or template literal is one
//! token, written as in the file, so that whitespace inside it counts. Text between JSX tags is one
//! token with its whitespace collapsed. In Python, where indentation is syntax, a token that starts
//! a line is preceded by a line break and the line's indentation instead of a space.

use std::ops::Range;

use tree_sitter::Node;

/// The kinds of node, across the grammars, taken whole as one token.
const LITERALS: [&str; 8] = [
    "string_literal",
    "raw_string_literal",
    "char_literal",
    "string",
    "template_string",
    "regex",
    "interpreted_string_literal",
    "rune_literal",
];

/// The canonical text of the tokens of `root` that lie within `span` of `text`.
pub(super) fn text(root: Node<'_>, text: &str, span: Range<usize>, python: bool) -> String {
    let mut canonical = Canonical {
        out: String::new(),
        last_row: None,
        python,
    };
    canonical.visit(root, text, &span);
    canonical.out
}

struct Canonical {
    out: String,
    /// The row on which the last token written ends.
    last_row: Option<usize>,
    python: bool,
}

impl Canonical {
    fn visit(&mut self, node: Node<'_>, text: &str, span: &Range<usize>) {
        if node.end_byte() <= span.start || node.start_byte() >= span.end {
            return;
        }
        let kind = node.kind();
        if kind.ends_with("comment") {
            return;
        }
        let inside = span.start <= node.start_byte() && node.end_byte() <= span.end;
        if inside && (LITERALS.contains(&kind) || node.child_count() == 0) {
            self.token(node, &text[node.byte_range()]);
            return;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit(child, text, span);
        }
    }

    fn token(&mut self, node: Node<'_>, token: &str) {
        let collapsed;
        let token = if node.kind() == "jsx_text" {
            collapsed = token.split_whitespace().collect::<Vec<_>>().join(" ");
            collapsed.as_str()
        } else {
            token
        };
        if token.is_empty() {
            return;
        }
        let start = node.start_position();
        match self.last_row {
            None => {}
            Some(row) if self.python && start.row > row => {
                self.out.push('\n');
                self.out.extend(std::iter::repeat_n(' ', start.column));
            }
            Some(_) => self.out.push(' '),
        }
        self.out.push_str(token);
        self.last_row = Some(node.end_position().row);
    }
}
