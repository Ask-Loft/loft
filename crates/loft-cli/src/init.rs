//! `loft init` (F-36): an empty document created in the current directory, with a fresh
//! document key. Refused, with nothing written, where `spec/` exists already (RQ-129).

use std::path::Path;
use std::process::ExitCode;

use loftspec::codec;
use loftspec::document::Document;
use loftspec::ids::DocumentId;
use loftspec_store::FileStore;
use serde_json::json;

use crate::json;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-init/1";

pub fn run(root: &Path, as_json: bool) -> ExitCode {
    let document = Document::empty(DocumentId::mint());
    match FileStore::open(root).and_then(|store| store.create(codec::write(&document))) {
        Ok(()) => {
            let key = document.id.to_string();
            if as_json {
                json::print(&json!({
                    "schema": SCHEMA,
                    "created": "spec/",
                    "documentKey": key,
                    "errors": [],
                }));
            } else {
                println!("created spec/, document key {key}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            let message = error.to_string();
            if as_json {
                json::print(&json!({
                    "schema": SCHEMA,
                    "errors": [json::error("store", &message, &[])],
                }));
            }
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
