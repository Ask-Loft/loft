//! `loft init` (F-36): an empty document created in the current directory, with a fresh
//! document key, and the `loft-spec` skill beside it unless `--no-skill` is given. Refused, with
//! nothing written, where `spec/` exists already (RQ-129).

use std::fmt::Display;
use std::path::Path;
use std::process::ExitCode;

use loftspec::codec;
use loftspec::document::Document;
use loftspec::ids::DocumentId;
use loftspec_store::FileStore;
use serde_json::{Value, json};

use crate::{json, skill};

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-init/1";

pub fn run(root: &Path, with_skill: bool, as_json: bool) -> ExitCode {
    let document = Document::empty(DocumentId::mint());
    if let Err(error) =
        FileStore::open(root).and_then(|store| store.create(codec::write(&document)))
    {
        return failed(&error, json::store_error(&error), as_json);
    }
    let skills = if with_skill {
        match skill::install(root) {
            Ok(skills) => skills,
            Err(error) => return failed(&error, skill::error_json(&error), as_json),
        }
    } else {
        Vec::new()
    };
    let key = document.id.to_string();
    if as_json {
        let fields = json!({ "created": "spec/", "documentKey": key, "skills": skills });
        json::print(&json::result(SCHEMA, "created", fields, Vec::new()));
    } else {
        println!("created spec/, document key {key}");
        for directory in skills {
            println!("installed {directory}");
        }
    }
    ExitCode::SUCCESS
}

/// Reports an error that stopped the command, as JSON with `value` and on standard error.
fn failed(error: &impl Display, value: Value, as_json: bool) -> ExitCode {
    if as_json {
        json::print(&json::failure(SCHEMA, vec![value]));
    }
    eprintln!("{error}");
    ExitCode::FAILURE
}
