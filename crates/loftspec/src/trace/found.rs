//! What the reconciler finds of each test a link names: the test located in its file's syntax
//! tree, with the lines of each location and the digest of its canonical texts; the test that a
//! part of its file that the grammar does not parse may cut short; the test found only as a whole
//! word; or the test not found. The reconciler parses no file: the port [`TestFiles`] reads each
//! file and gives the tests it holds and the parts its grammar does not parse, and each file is
//! read and parsed once.

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;
use std::path::{Component, Path};

use super::Test;
use crate::canonical::{Digest, digest_bytes};

/// The files that hold the tests, by their paths relative to the directory that holds `spec/`.
pub trait TestFiles {
    fn read(&self, path: &str) -> TestFile;

    /// The tests that `text`, the file at `path`, holds, and the parts of it that its grammar
    /// does not parse; none where no grammar parses the file. Without a parser, none.
    fn tests(&self, path: &str, text: &str) -> Option<ParsedFile> {
        let _ = (path, text);
        None
    }
}

/// What reading a test file gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestFile {
    Text(String),
    /// No file is at the path.
    Missing,
    /// The file is not UTF-8 text.
    NotText,
    /// The file is there but cannot be read, and why.
    Unreadable(String),
}

/// What a grammar found in a file: the tests it holds, in the order of the file, and the parts
/// of it that the grammar does not parse, in the order of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParsedFile {
    pub tests: Vec<LocatedTest>,
    pub errors: Vec<ParseError>,
}

impl ParsedFile {
    #[must_use]
    pub const fn new(tests: Vec<LocatedTest>, errors: Vec<ParseError>) -> Self {
        Self { tests, errors }
    }
}

/// One test that a parsed file holds.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LocatedTest {
    pub name: String,
    pub lines: Lines,
    /// The bytes it spans in the text the grammar parsed.
    pub bytes: Range<usize>,
    /// Its tokens in the order of the file, joined by one space, without comments.
    pub canonical: String,
    /// Its text in the file.
    pub source: String,
}

impl LocatedTest {
    #[must_use]
    pub const fn new(
        name: String,
        lines: Lines,
        bytes: Range<usize>,
        canonical: String,
        source: String,
    ) -> Self {
        Self {
            name,
            lines,
            bytes,
            canonical,
            source,
        }
    }
}

/// A part of a file that its grammar does not parse: text it could not read, or a token it found
/// missing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseError {
    /// The byte where it starts in the text the grammar parsed.
    pub at: usize,
    /// Its line and its column, in characters, each counted from 1.
    pub line: usize,
    pub column: usize,
    /// The start of the text it could not read, or the token it found missing.
    pub text: String,
    /// Whether `text` is a token that the grammar found missing.
    pub missing: bool,
}

impl ParseError {
    #[must_use]
    pub const fn new(at: usize, line: usize, column: usize, text: String, missing: bool) -> Self {
        Self {
            at,
            line,
            column,
            text,
            missing,
        }
    }
}

impl fmt::Display for ParseError {
    /// Where the part is, and its text or the token missing: ``3:1, at `1)` `` or ``2:15, where
    /// `)` is missing``.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (line, column, text) = (self.line, self.column, &self.text);
        if self.missing {
            write!(formatter, "{line}:{column}, where `{text}` is missing")
        } else {
            write!(formatter, "{line}:{column}, at `{text}`")
        }
    }
}

/// The first and last line of a test's location, counted from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Lines {
    pub first: usize,
    pub last: usize,
}

impl Lines {
    #[must_use]
    pub const fn new(first: usize, last: usize) -> Self {
        Self { first, last }
    }
}

/// What the reconciler found of one test; a later version may add outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Found {
    /// Located: the lines of each location, in the order of the file, and the test's digest.
    Located { lines: Vec<Lines>, digest: Digest },
    /// Found only as a whole word, and why it was not located.
    Word(Untracked),
    /// Not located, since a part of its file that the grammar does not parse may cut it short:
    /// the first such part.
    Unparsed(ParseError),
    /// Not found, and why: the message of its `TR-004` finding.
    Missing(String),
}

/// Why a test found as a whole word was not located; a later version may add reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Untracked {
    /// No grammar parses the file.
    NoGrammar,
    /// No test of the parsed file carries the test's name; with the first part of the file that
    /// the grammar does not parse, where one does not.
    NoTest(Option<ParseError>),
}

/// The test files, each read and parsed once.
pub(super) struct Seeker<'a, F> {
    files: &'a F,
    read: HashMap<String, Read>,
}

/// A file as read, and what a grammar found in it where one parsed it.
struct Read {
    file: TestFile,
    parsed: Option<ParsedFile>,
}

impl<'a, F: TestFiles> Seeker<'a, F> {
    pub(super) fn new(files: &'a F) -> Self {
        Self {
            files,
            read: HashMap::new(),
        }
    }

    /// What is found of `test`.
    pub(super) fn seek(&mut self, test: &Test) -> Found {
        let file = test.file.as_str();
        if !inside(file) {
            return Found::Missing(format!(
                "the file `{file}` lies outside the directory that holds `spec/`"
            ));
        }
        let read = self.read(file);
        let held = match &read.file {
            TestFile::Text(held) => held,
            TestFile::Missing => {
                return Found::Missing(format!("the file `{file}` does not exist"));
            }
            TestFile::NotText => {
                return Found::Missing(format!("the file `{file}` is not UTF-8 text"));
            }
            TestFile::Unreadable(why) => {
                return Found::Missing(format!("the file `{file}` cannot be read: {why}"));
            }
        };
        let parsed = read.parsed.as_ref();
        let located: Vec<&LocatedTest> = named(parsed, &test.name).collect();
        if let Some(error) = parsed.and_then(|parsed| {
            located
                .iter()
                .find_map(|located| cut_short(parsed, located))
        }) {
            return Found::Unparsed(error.clone());
        }
        if !located.is_empty() {
            let canonical: Vec<&str> = located.iter().map(|test| test.canonical.as_str()).collect();
            return Found::Located {
                lines: located.iter().map(|test| test.lines).collect(),
                digest: digest_bytes(canonical.join("\n").as_bytes()),
            };
        }
        if !occurs_whole(held, &test.name) {
            return Found::Missing(format!(
                "`{}` does not occur in `{file}` as a whole word",
                test.name
            ));
        }
        Found::Word(parsed.map_or(Untracked::NoGrammar, |parsed| {
            Untracked::NoTest(parsed.errors.first().cloned())
        }))
    }

    /// Whether the source of `test`, of which `found` was found, holds `literal`: the text of its
    /// locations where it was located, its whole file where it was found only as a word.
    pub(super) fn holds(&self, test: &Test, found: &Found, literal: &str) -> bool {
        let Some(read) = self.read.get(&test.file) else {
            return false;
        };
        match (found, &read.file) {
            (Found::Located { .. }, _) => named(read.parsed.as_ref(), &test.name)
                .any(|located| located.source.contains(literal)),
            (Found::Word(_) | Found::Unparsed(_), TestFile::Text(held)) => held.contains(literal),
            _ => false,
        }
    }

    fn read(&mut self, file: &str) -> &Read {
        let files = self.files;
        self.read.entry(file.to_owned()).or_insert_with(|| {
            let read = files.read(file);
            let parsed = match &read {
                TestFile::Text(text) => files.tests(file, text),
                _ => None,
            };
            Read { file: read, parsed }
        })
    }
}

/// The tests of a parsed file that carry `name`, in the order of the file.
fn named<'a>(
    parsed: Option<&'a ParsedFile>,
    name: &'a str,
) -> impl Iterator<Item = &'a LocatedTest> {
    parsed
        .map_or(&[][..], |parsed| &parsed.tests)
        .iter()
        .filter(move |test| test.name == name)
}

/// The first part of `parsed` that the grammar does not parse and that may cut `test` short: one
/// that `test` holds, or one that follows it before the next test of the file starts.
fn cut_short<'a>(parsed: &'a ParsedFile, test: &LocatedTest) -> Option<&'a ParseError> {
    let Range { start, end } = test.bytes;
    parsed.errors.iter().find(|error| {
        start <= error.at
            && (error.at < end
                || !parsed
                    .tests
                    .iter()
                    .any(|other| (end..=error.at).contains(&other.bytes.start)))
    })
}

/// Whether `path` names a file under the directory that holds `spec/`: relative, and never
/// stepping up out of it, though it may step up and back down inside it.
fn inside(path: &str) -> bool {
    let mut depth = 0_usize;
    Path::new(path).components().all(|part| match part {
        Component::Normal(_) => {
            depth += 1;
            true
        }
        Component::CurDir => true,
        Component::ParentDir => depth.checked_sub(1).map(|up| depth = up).is_some(),
        Component::RootDir | Component::Prefix(_) => false,
    })
}

/// Whether `name` occurs in `text` where no letter, digit or underscore adjoins either end. Every
/// place it starts is tried, those that overlap an earlier one included.
fn occurs_whole(text: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let word = |character: char| character.is_alphanumeric() || character == '_';
    let mut from = 0;
    while let Some(found) = text[from..].find(name) {
        let at = from + found;
        let before = text[..at].chars().next_back();
        let after = text[at + name.len()..].chars().next();
        if !before.is_some_and(word) && !after.is_some_and(word) {
            return true;
        }
        // The next place it may start is one character on, so an overlapping one is tried.
        from = at + text[at..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// The literals of a node's name: the texts between two backticks, each once, in the order of
/// the name.
pub(super) fn literals(name: &str) -> Vec<&str> {
    let parts: Vec<&str> = name.split('`').collect();
    let mut literals: Vec<&str> = Vec::new();
    // The odd parts lie between two backticks, except a last part that no backtick closes.
    for part in parts.iter().take(parts.len() - 1).skip(1).step_by(2) {
        if !part.is_empty() && !literals.contains(part) {
            literals.push(part);
        }
    }
    literals
}
