//! The manifest's format and algorithm (F-5).

use loftspec::FORMAT;
use loftspec::manifest::{ALGORITHM, ManifestError, check_identifiers};

#[test]
fn the_format_and_algorithm_identifiers() {
    assert_eq!(FORMAT, "loftspec/1");
    assert_eq!(ALGORITHM, "sha256-jcs/1");
    assert_eq!(check_identifiers(FORMAT, ALGORITHM), Ok(()));
}

#[test]
fn an_unknown_format_is_refused_naming_it() {
    let error = check_identifiers("loftspec/2", ALGORITHM).unwrap_err();
    assert_eq!(error, ManifestError::UnknownFormat("loftspec/2".to_owned()));
    assert!(error.to_string().contains("loftspec/2"));
}

#[test]
fn an_unknown_algorithm_is_refused_naming_it() {
    let error = check_identifiers(FORMAT, "blake3-jcs/1").unwrap_err();
    assert_eq!(
        error,
        ManifestError::UnknownAlgorithm("blake3-jcs/1".to_owned())
    );
    assert!(error.to_string().contains("blake3-jcs/1"));
}
