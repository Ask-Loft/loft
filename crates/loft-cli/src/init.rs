//! `loft init` (F-36): an empty document created in the current directory, with a fresh
//! document key, and the `loft-spec` skill beside it unless `--no-skill` is given. Refused, with
//! nothing written, where `spec/` exists already (RQ-129).

use std::path::Path;
use std::process::ExitCode;

use loftspec::codec;
use loftspec::document::Document;
use loftspec::ids::DocumentId;
use loftspec_store::FileStore;
use serde_json::json;

use crate::{json, skill};

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-init/1";

pub fn run(root: &Path, with_skill: bool, as_json: bool) -> ExitCode {
    let document = Document::empty(DocumentId::mint());
    if let Err(error) =
        FileStore::open(root).and_then(|store| store.create(codec::write(&document)))
    {
        return json::stop(SCHEMA, vec![json::store_error(&error)], &error, as_json);
    }
    let skills = if with_skill {
        match skill::install(root) {
            Ok(skills) => skills,
            Err(error) => {
                return json::stop(SCHEMA, vec![skill::error_json(&error)], &error, as_json);
            }
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
        skill::report(&skills);
    }
    ExitCode::SUCCESS
}
