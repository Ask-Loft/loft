//! The Loft Specification Format: document model, identity, canonical bytes, hashing and
//! conversion from the v1 export. This crate performs no I/O.

pub mod attributes;
pub mod bindings;
pub mod canonical;
pub mod decision;
pub mod hash;
pub mod identity;
pub mod kind;
pub mod manifest;
pub mod model;
pub mod statement;

/// The identifier a manifest names for this format, versioned with the text form it describes.
pub const FORMAT: &str = "loftspec/1";

#[cfg(test)]
mod tests {
    use super::FORMAT;

    #[test]
    fn format_identifier_names_the_first_version() {
        assert_eq!(FORMAT, "loftspec/1");
    }
}
