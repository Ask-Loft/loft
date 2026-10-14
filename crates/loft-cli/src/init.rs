//! `loft init` (F-36): an empty document created in the current directory, with a fresh
//! document key, and the `loft-spec` skill beside it unless `--no-skill` is given. Refused, with
//! nothing written, where `spec/` exists already (RQ-129). Inside a git repository whose local
//! configuration does not register the merge driver, the text output suggests registering it;
//! outside any repository, it suggests putting the project under git first. A git that cannot
//! be run or fails leaves the suggestion out and never fails the command.

use std::path::Path;
use std::process::ExitCode;

use loftspec::codec;
use loftspec::document::Document;
use loftspec::ids::DocumentId;
use loftspec_store::merge_driver::DRIVER;
use loftspec_store::repository::GitCommand;
use loftspec_store::{FileStore, StoreError};
use serde_json::json;

use crate::merge_driver::NPM_COMMAND;
use crate::out::outln;
use crate::stop::Stop;
use crate::{json, skill};

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-init/1";

pub fn run(root: &Path, with_skill: bool, as_json: bool) -> ExitCode {
    let document = Document::empty(DocumentId::mint());
    if let Err(error) =
        FileStore::open(root).and_then(|store| store.create(codec::write(&document)))
    {
        return Stop::from(error).exit(SCHEMA, as_json);
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
        outln!("created spec/, document key {key}");
        skill::report(&skills);
        if let Some(suggestion) = merge_driver_suggestion(&GitCommand::default(), root) {
            outln!("{suggestion}");
        }
    }
    ExitCode::SUCCESS
}

/// What to do about the merge driver: register it where the repository has not, or put the
/// project under git first; nothing where it is registered or git cannot answer.
fn merge_driver_suggestion(git: &GitCommand, root: &Path) -> Option<String> {
    match git.config_get(root, DRIVER) {
        Ok(None) => Some(format!(
            "next: run `loft merge-driver --install` once in each clone, so git merges spec/ by \
             key; where loft runs through npm, add --command \"{NPM_COMMAND}\""
        )),
        Ok(Some(_)) => None,
        // Outside a repository git refuses the read of its local configuration.
        Err(_) => matches!(git.prefix(root), Err(StoreError::NotARepository(_))).then(|| {
            "next: the merge driver needs a git repository; put the project under git, then run \
             `loft merge-driver --install` once in each clone"
                .to_owned()
        }),
    }
}
