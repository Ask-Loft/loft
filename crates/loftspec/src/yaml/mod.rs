//! The YAML subset of the text form (D-11 F-9, ADR-14): a strict reader that accepts block
//! mappings and sequences with plain, double-quoted and literal scalars, and refuses every other
//! construct at its line and column; and the canonical writer, whose output the reader reads
//! back byte for byte.

mod emit;
mod error;
mod parse;
mod scalar;
mod source;

pub use emit::{Out, write};
pub use error::{Error, ErrorKind, Location, Position};

/// A value with the position of its first character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub position: Position,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Entries in the order written; no key repeats.
    Mapping(Vec<Entry>),
    Sequence(Vec<Node>),
    Scalar(Scalar),
    /// A key with no value after it.
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub key_position: Position,
    pub value: Node,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scalar {
    pub style: Style,
    pub text: String,
}

/// The scalar styles the subset admits (AC-113).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Plain,
    DoubleQuoted,
    Literal,
}

/// Reads one file of the subset: `None` for a file with no content, which is how an empty
/// registry is written.
///
/// # Errors
///
/// The first construct outside the subset, with its location (RQ-30..RQ-33, RQ-65).
pub fn read(bytes: &[u8]) -> Result<Option<Node>, Error> {
    let text = source::decode(bytes)?;
    parse::document(text)
}
