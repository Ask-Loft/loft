//! The test locator (the Feature "Locating a test"): the part of `loft` that parses a test file
//! with tree-sitter and gives the reconciler the tests it holds, each with its lines, its bytes,
//! its canonical text and its text in the file, and the parts of the file the grammar does not
//! parse. A grammar is chosen by the file's extension; a file of any other extension is not
//! parsed, and the reconciler then looks for its tests as words.

mod canonical;
mod go;
mod python;
mod rust;
mod script;
mod strings;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::HashMap;

use loftspec::trace::{Lines, LocatedTest, ParseError, ParsedFile, TestFile, TestFiles};
use tree_sitter::{Language, Node, Parser};

/// The test files `files` reads, each parsed by the test locator once, however often the
/// reconciler asks: the files do not change during a run.
pub struct Parsed<F> {
    files: F,
    parsed: RefCell<HashMap<String, Option<ParsedFile>>>,
}

impl<F> Parsed<F> {
    pub fn new(files: F) -> Self {
        Self {
            files,
            parsed: RefCell::default(),
        }
    }
}

impl<F: TestFiles> TestFiles for Parsed<F> {
    fn read(&self, path: &str) -> TestFile {
        self.files.read(path)
    }

    fn tests(&self, path: &str, text: &str) -> Option<ParsedFile> {
        self.parsed
            .borrow_mut()
            .entry(path.to_owned())
            .or_insert_with(|| locate(path, text))
            .clone()
    }
}

/// A grammar the test locator parses with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grammar {
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
}

impl Grammar {
    /// The grammar of the file at `path`, by its extension.
    fn of(path: &str) -> Option<Self> {
        let (_, extension) = path.rsplit_once('.')?;
        Some(match extension {
            "rs" => Self::Rust,
            "ts" | "mts" | "cts" => Self::TypeScript,
            "tsx" => Self::Tsx,
            "js" | "mjs" | "cjs" | "jsx" => Self::JavaScript,
            "py" => Self::Python,
            "go" => Self::Go,
            _ => return None,
        })
    }

    fn language(self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
        }
    }
}

/// A test the grammar's rules found: its name, and the bytes and rows it spans.
struct Candidate {
    name: String,
    start: usize,
    end: usize,
    first_row: usize,
    last_row: usize,
}

impl Candidate {
    /// The candidate that spans `node`, named `name`.
    fn spanning(name: String, node: Node<'_>) -> Self {
        Self::between(name, node, node)
    }

    /// The candidate that spans from the start of `first` to the end of `last`.
    fn between(name: String, first: Node<'_>, last: Node<'_>) -> Self {
        Self {
            name,
            start: first.start_byte(),
            end: last.end_byte(),
            first_row: first.start_position().row,
            last_row: last.end_position().row,
        }
    }
}

/// The tests that `text`, the file at `path`, holds, in the order of the file, and the parts of it
/// the grammar does not parse; none where no grammar parses the file. A line break written as
/// CR LF is read as LF, so a checkout's line endings change no digest.
pub fn locate(path: &str, text: &str) -> Option<ParsedFile> {
    let grammar = Grammar::of(path)?;
    let unix;
    let text = if text.contains("\r\n") {
        unix = text.replace("\r\n", "\n");
        unix.as_str()
    } else {
        text
    };
    let mut parser = Parser::new();
    parser.set_language(&grammar.language()).ok()?;
    let tree = parser.parse(text, None)?;
    let root = tree.root_node();
    let mut candidates = match grammar {
        Grammar::Rust => rust::tests(root, text),
        Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => script::tests(root, text),
        Grammar::Python => python::tests(root, text),
        Grammar::Go => go::tests(root, text),
    };
    candidates.sort_by_key(|candidate| (candidate.start, candidate.end));
    let python = grammar == Grammar::Python;
    let tests = candidates
        .into_iter()
        .map(|candidate| {
            let bytes = candidate.start..candidate.end;
            LocatedTest::new(
                candidate.name,
                Lines::new(candidate.first_row + 1, candidate.last_row + 1),
                bytes.clone(),
                canonical::text(root, text, bytes.clone(), python),
                text[bytes].to_owned(),
            )
        })
        .collect();
    Some(ParsedFile::new(tests, errors(root, text)))
}

/// The longest start of a part that does not parse that is given, in characters.
const SHOWN: usize = 40;

/// The parts of the tree under `root` that the grammar does not parse, in the order of the file:
/// each node it could not read, without the parts inside it, and each token it found missing.
fn errors(root: Node<'_>, text: &str) -> Vec<ParseError> {
    let mut errors = Vec::new();
    let mut cursor = root.walk();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            errors.push(error(node, text));
        } else if node.has_error() {
            let children: Vec<Node<'_>> = node.children(&mut cursor).collect();
            stack.extend(children.into_iter().rev());
        }
    }
    errors
}

/// The part `node` that the grammar does not parse: where it starts, and its first line, or the
/// token found missing.
fn error(node: Node<'_>, text: &str) -> ParseError {
    let at = node.start_byte();
    let start = node.start_position();
    let column = text[at - start.column..at].chars().count() + 1;
    let (shown, missing) = if node.is_missing() {
        (node.kind().to_owned(), true)
    } else {
        let first = text_of(node, text).lines().next().unwrap_or_default();
        (first.chars().take(SHOWN).collect(), false)
    };
    ParseError::new(at, start.row + 1, column, shown, missing)
}

/// `node` without the parentheses around it, however many.
fn unparenthesized(mut node: Node<'_>) -> Node<'_> {
    while node.kind() == "parenthesized_expression" {
        let mut cursor = node.walk();
        let inner = node
            .named_children(&mut cursor)
            .find(|inner| !inner.kind().ends_with("comment"));
        match inner {
            Some(inner) => node = inner,
            None => break,
        }
    }
    node
}

/// The text of `node` in `text`.
fn text_of<'a>(node: Node<'_>, text: &'a str) -> &'a str {
    &text[node.byte_range()]
}

/// Every node of the tree under `root`, `root` first, each before its children.
fn each(root: Node<'_>, visit: &mut impl FnMut(Node<'_>)) {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        visit(node);
        let children: Vec<Node<'_>> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
}
