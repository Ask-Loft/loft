//! The manifest: the format and algorithm a document is written in (D-11 F-5).

use crate::FORMAT;

/// The identifier a manifest names for the hashing algorithm: SHA-256 over RFC 8785 canonical
/// JSON with domain separation.
pub const ALGORITHM: &str = "sha256-jcs/1";

/// A manifest identifier this implementation does not implement (RQ-18).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("the manifest names format `{0}`; this implementation reads `{FORMAT}`")]
    UnknownFormat(String),
    #[error("the manifest names algorithm `{0}`; this implementation computes `{ALGORITHM}`")]
    UnknownAlgorithm(String),
}

/// Refuses a manifest whose format or algorithm this implementation does not implement.
pub fn check_identifiers(format: &str, algorithm: &str) -> Result<(), ManifestError> {
    if format != FORMAT {
        return Err(ManifestError::UnknownFormat(format.to_owned()));
    }
    if algorithm != ALGORITHM {
        return Err(ManifestError::UnknownAlgorithm(algorithm.to_owned()));
    }
    Ok(())
}
