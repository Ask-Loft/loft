//! The digest Yard Client names its events, tasks and receipts by.

use serde_json::Value;
use sha2::{Digest as _, Sha256};

/// The SHA-256 of `value`'s compact JSON text, in lowercase hex.
pub fn json_hex(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}
