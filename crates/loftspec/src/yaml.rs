//! The YAML subset of the text form (F-9, ADR-14), read and written by `loft-common`: a strict
//! reader that accepts block mappings and sequences with plain, double-quoted and literal
//! scalars, and refuses every other construct at its line and column; and the canonical writer,
//! whose output the reader reads back byte for byte.

pub use loft_common::yaml::{
    Entry, Error, ErrorKind, Location, Node, Out, Position, Scalar, Style, Value, read, write,
};
pub(crate) use loft_common::yaml::{is_key, utf8, write_keyed};
