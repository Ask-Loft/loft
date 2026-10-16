//! A test's canonical text: its tokens in the order of the file, joined by one space, without
//! comments. A string, character, raw string, regular expression or template literal is one
//! token, written as in the file, so that whitespace inside it counts. Text between JSX tags is one
//! token with its whitespace collapsed. A comma directly before `)`, `]` or `}` is dropped, since
//! formatters add and remove it, except in Python, where `(x,)` is a tuple. In Python, where
//! indentation is syntax, a token that starts a line outside any bracket is preceded by a line
//! break and the line's indentation as written instead of a space, and a backslash that continues
//! a line is whitespace.

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
        text,
        out: String::new(),
        last_row: None,
        comma_at: None,
        depth: 0,
        continued: false,
        python,
    };
    canonical.visit(root, &span);
    canonical.out
}

struct Canonical<'a> {
    text: &'a str,
    out: String,
    /// The row on which the last token written ends.
    last_row: Option<usize>,
    /// Where the separator before the last token begins, when that token is a comma.
    comma_at: Option<usize>,
    /// How many brackets are open, counted in Python only.
    depth: usize,
    /// Whether a backslash continued the line since the last token.
    continued: bool,
    python: bool,
}

impl Canonical<'_> {
    /// The tokens of `node` within `span`, from the first child that reaches into it to the last
    /// that starts inside it, so a test costs its own size, not the file's.
    fn visit(&mut self, node: Node<'_>, span: &Range<usize>) {
        if node.end_byte() <= span.start || node.start_byte() >= span.end {
            return;
        }
        let kind = node.kind();
        if kind == "line_continuation" {
            self.continued = true;
            return;
        }
        if kind.ends_with("comment") {
            return;
        }
        let inside = span.start <= node.start_byte() && node.end_byte() <= span.end;
        if inside && (LITERALS.contains(&kind) || node.child_count() == 0) {
            self.token(node);
            return;
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child_for_byte(span.start).is_none() {
            return;
        }
        loop {
            let child = cursor.node();
            if child.start_byte() >= span.end {
                break;
            }
            self.visit(child, span);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn token(&mut self, node: Node<'_>) {
        let written = &self.text[node.byte_range()];
        let collapsed;
        let token = if node.kind() == "jsx_text" {
            collapsed = written.split_whitespace().collect::<Vec<_>>().join(" ");
            collapsed.as_str()
        } else {
            written
        };
        if token.is_empty() {
            return;
        }
        if !self.python
            && matches!(token, ")" | "]" | "}")
            && let Some(at) = self.comma_at.take()
        {
            self.out.truncate(at);
        }
        let separator_at = self.out.len();
        self.separate(node);
        self.out.push_str(token);
        self.comma_at = (token == ",").then_some(separator_at);
        self.last_row = Some(node.end_position().row);
        if self.python {
            match token {
                "(" | "[" | "{" => self.depth += 1,
                ")" | "]" | "}" => self.depth = self.depth.saturating_sub(1),
                _ => {}
            }
        }
    }

    /// What comes before the token `node`: nothing first, a line break and the line's indentation
    /// where a Python line starts outside any bracket, and one space otherwise.
    fn separate(&mut self, node: Node<'_>) {
        let start = node.start_position();
        match self.last_row {
            None => {}
            Some(row) if self.python && self.depth == 0 && !self.continued && start.row > row => {
                let line_start = node.start_byte() - start.column;
                self.out.push('\n');
                self.out.push_str(&self.text[line_start..node.start_byte()]);
            }
            Some(_) => self.out.push(' '),
        }
        self.continued = false;
    }
}
