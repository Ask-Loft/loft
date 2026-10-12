//! The Loft Specification Format: document model, identity, canonical bytes, hashing and
//! conversion from the v1 export. This crate performs no I/O.

pub mod attributes;
pub mod bindings;
pub mod boundary;
pub mod canonical;
pub mod check;
pub mod codec;
pub mod convert;
pub mod decision;
pub mod document;
mod ears;
pub mod fmt;
pub mod hash;
pub mod identity;
pub mod ids;
pub mod index;
pub mod kind;
pub mod lint;
pub mod manifest;
pub mod names;
pub mod node;
pub mod prose;
pub mod render;
pub mod slug;
pub mod statement;
pub mod text;
pub mod tree;
pub mod validate;
mod wire;
pub mod yaml;

/// The identifier a manifest names for this format, versioned with the text form it describes.
pub const FORMAT: &str = "loftspec/1";
