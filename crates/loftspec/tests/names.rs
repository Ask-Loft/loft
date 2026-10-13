//! Tag and constraint category names (D-11 RQ-71, RQ-72): a tag name is a lowercase letter
//! followed by lowercase letters, digits and hyphens; a category name is one or more such
//! segments joined by dots.

use loftspec::names::{CategoryName, NameError, TagName};

#[test]
fn a_tag_name_such_as_conversion_is_accepted() {
    for name in ["conversion", "git", "a", "v2-lint", "x9"] {
        assert_eq!(TagName::parse(name).unwrap().as_str(), name);
    }
}

#[test]
fn a_tag_name_with_an_uppercase_letter_is_an_error_naming_it() {
    let error = TagName::parse("Conversion").unwrap_err();
    assert_eq!(error, NameError::Tag("Conversion".to_owned()));
    assert!(error.to_string().contains("`Conversion`"), "{error}");
}

#[test]
fn a_tag_name_containing_a_dot_is_an_error() {
    assert!(TagName::parse("runtime.language").is_err());
}

#[test]
fn a_tag_name_that_begins_with_a_digit_or_a_hyphen_is_an_error() {
    for name in ["9lives", "-git", ""] {
        assert!(TagName::parse(name).is_err(), "{name:?}");
    }
}

#[test]
fn a_category_name_such_as_runtime_language_is_accepted() {
    for name in ["runtime.language", "runtime", "a.b-c.d9"] {
        assert_eq!(CategoryName::parse(name).unwrap().as_str(), name);
    }
}

#[test]
fn a_category_name_with_an_uppercase_letter_is_an_error_naming_it() {
    let error = CategoryName::parse("runtime.Language").unwrap_err();
    assert_eq!(error, NameError::Category("runtime.Language".to_owned()));
    assert!(error.to_string().contains("`runtime.Language`"), "{error}");
}

#[test]
fn a_category_name_with_an_empty_segment_is_an_error() {
    for name in ["runtime..language", ".runtime", "runtime.", ""] {
        assert!(CategoryName::parse(name).is_err(), "{name:?}");
    }
}

#[test]
fn a_category_name_segment_that_begins_with_a_digit_or_a_hyphen_is_an_error() {
    for name in ["runtime.9lives", "runtime.-x", "1.runtime"] {
        assert!(CategoryName::parse(name).is_err(), "{name:?}");
    }
}
