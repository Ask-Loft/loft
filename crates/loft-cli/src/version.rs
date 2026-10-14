//! `loft --version` (F-46): the tool release, the format and the hashing algorithm, on three
//! lines (AC-589..AC-591) or as one JSON document (AC-682).

use loftspec::FORMAT;
use loftspec::manifest::ALGORITHM;
use serde_json::json;

use crate::json;
use crate::out::outln;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-version/1";

/// The tool release, from the crate that builds `loft`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(as_json: bool) {
    if as_json {
        json::print(&json!({
            "schema": SCHEMA,
            "version": VERSION,
            "format": FORMAT,
            "algorithm": ALGORITHM,
        }));
    } else {
        outln!("loft {VERSION}");
        outln!("format {FORMAT}");
        outln!("algorithm {ALGORITHM}");
    }
}
