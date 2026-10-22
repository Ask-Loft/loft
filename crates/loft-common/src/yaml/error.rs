//! Where a read error is and what it is.

use std::fmt;

/// A 1-based line and a 1-based column counted in Unicode scalar values (AC-680, AC-681).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    #[must_use]
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.line, self.column)
    }
}

/// A line and column, or, for bytes that are not UTF-8, a byte offset (RQ-65).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Location {
    At(Position),
    Byte(usize),
}

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::At(position) => position.fmt(formatter),
            Self::Byte(offset) => write!(formatter, "byte {offset}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{location}: {kind}")]
pub struct Error {
    pub location: Location,
    pub kind: ErrorKind,
}

impl Error {
    pub(super) const fn at(position: Position, kind: ErrorKind) -> Self {
        Self {
            location: Location::At(position),
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorKind {
    #[error("the bytes are not UTF-8")]
    InvalidUtf8,
    #[error("a byte order mark")]
    ByteOrderMark,
    #[error("a carriage return; lines end with a line feed only")]
    CarriageReturn,
    #[error("the character U+{0:04X} is not printable")]
    NotPrintable(u32),
    #[error("a tab in indentation")]
    TabInIndentation,
    #[error("a tab where a space separates")]
    TabSeparator,
    #[error("a comment")]
    Comment,
    #[error("an anchor")]
    Anchor,
    #[error("an alias")]
    Alias,
    #[error("a tag")]
    Tag,
    #[error("a flow collection")]
    FlowCollection,
    #[error("a folded scalar")]
    FoldedScalar,
    #[error("a single-quoted scalar")]
    SingleQuotedScalar,
    #[error("a document marker or directive")]
    DocumentMarker,
    #[error("a reserved indicator `{0}`")]
    ReservedIndicator(char),
    #[error("a complex key")]
    ComplexKey,
    #[error("the field `{0}` is repeated")]
    DuplicateKey(String),
    #[error("`{0}` is not a field name")]
    InvalidKey(String),
    #[error("indentation the structure does not expect")]
    UnexpectedIndentation,
    #[error("a sequence entry where a field is expected")]
    UnexpectedSequence,
    #[error("a field where a sequence entry is expected")]
    UnexpectedMapping,
    #[error("a sequence entry on the line of another")]
    CompactSequence,
    #[error("a plain scalar that continues on the next line")]
    MultiLinePlainScalar,
    #[error("a double-quoted scalar that is not closed on its line")]
    UnclosedQuote,
    #[error("an escape the double-quoted style does not define")]
    InvalidEscape,
    #[error("text after a closing quote")]
    TrailingText,
    #[error("a block scalar header the literal style does not define")]
    InvalidBlockHeader,
    #[error("a leading empty line of a literal block is indented past its first line")]
    OverIndentedEmptyLine,
}
