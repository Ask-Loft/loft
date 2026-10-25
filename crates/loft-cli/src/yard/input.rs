//! The JSON a command reads from `--input`: a file, or standard input for `-`. Input that cannot
//! be read, or is not JSON, is a usage error that names `--input`.

use std::fs;
use std::io::{self, Read as _};
use std::path::Path;

use serde_json::Value;

use super::failure::Failure;

pub fn json(input: &Path) -> Result<Value, Failure> {
    let text = if input == Path::new("-") {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| Failure::Usage(format!("--input: standard input: {error}")))?;
        text
    } else {
        fs::read_to_string(input)
            .map_err(|error| Failure::Usage(format!("--input: {}: {error}", input.display())))?
    };
    serde_json::from_str(&text)
        .map_err(|error| Failure::Usage(format!("--input: not JSON: {error}")))
}
