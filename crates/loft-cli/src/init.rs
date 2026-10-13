//! `loft init` (F-36): an empty document created in the current directory, with a fresh
//! document key, and the `loft-spec` skill beside it unless `--no-skill` is given. Refused, with
//! nothing written, where `spec/` exists already (RQ-129). Inside a git repository whose local
//! configuration does not register the merge driver, the text output suggests registering it.
//! A configuration git cannot read leaves the suggestion out and never fails the command.

use std::path::Path;
use std::process::ExitCode;

use loftspec::codec;
use loftspec::document::Document;
use loftspec::ids::DocumentId;
use loftspec_store::FileStore;
use loftspec_store::merge_driver::DRIVER;
use loftspec_store::repository::GitCommand;
use serde_json::json;

use crate::merge_driver::NPM_COMMAND;
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
        // Outside a repository git refuses the read, which leaves the suggestion out.
        if matches!(GitCommand::default().config_get(root, DRIVER), Ok(None)) {
            println!(
                "next: run `loft merge-driver --install` once in each clone, so git merges spec/ \
                 by key; where loft runs through npm, add --command \"{NPM_COMMAND}\""
            );
        }
    }
    ExitCode::SUCCESS
}
