//! Conversion into the Format from other representations (FG-4).

pub mod openspec;
pub mod v1;

/// A file a conversion writes beside `spec/`: its name and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarFile {
    pub name: &'static str,
    pub bytes: Vec<u8>,
}
