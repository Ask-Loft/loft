//! The environment variables `loft` reads. A variable set to the empty string counts as unset,
//! so `LOFT_TELEMETRY=` reads as no setting at all.

use std::env;
use std::ffi::OsString;

/// The value of the environment variable `name`, when it is set and not empty.
pub fn value(name: &str) -> Option<OsString> {
    env::var_os(name).filter(|value| !value.is_empty())
}

/// [`value`] as text, when it is also valid Unicode.
pub fn text(name: &str) -> Option<String> {
    value(name)?.into_string().ok()
}
