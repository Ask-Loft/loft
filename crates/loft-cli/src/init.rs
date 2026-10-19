//! `loft init` (F-36): an empty document created in the current directory, with a fresh
//! document key, and the `loft-spec` skill beside it unless `--no-skill` is given, with the
//! plugin enabled unless `--no-plugin` is given. Refused, with
//! nothing written, where `spec/` exists already (RQ-129). Inside a git repository whose local
//! configuration does not register the merge driver, the text output suggests registering it;
//! outside any repository, it suggests putting the project under git first. A git that cannot
//! be run or fails leaves the suggestion out and never fails the command.

use std::path::Path;

use loftspec::codec;
use loftspec::document::Document;
use loftspec::ids::DocumentId;
use loftspec_store::FileStore;
use loftspec_store::repository::GitCommand;
use serde_json::json;

use crate::finish::Finished;
use crate::out::outln;
use crate::stop::Stop;
use crate::{json, merge_driver, skill};

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-init/1";

/// Runs `loft init` in `root`, writing the skill where `skill` names how.
pub fn run(root: &Path, skill: Option<skill::Plugin>, as_json: bool) -> Finished {
    let document = Document::empty(DocumentId::mint());
    if let Err(error) =
        FileStore::open(root).and_then(|store| store.create(codec::write(&document)))
    {
        return Stop::from(error).exit(SCHEMA, as_json);
    }
    let installed = match skill.map(|plugin| skill::install(root, plugin)) {
        Some(Ok(installed)) => Some(installed),
        Some(Err(error)) => {
            return json::stop(SCHEMA, vec![skill::error_json(&error)], &error, as_json);
        }
        None => None,
    };
    let skills = installed
        .as_ref()
        .map_or_else(Vec::new, |installed| installed.skills.clone());
    let key = document.id.to_string();
    if as_json {
        let fields = json!({ "created": "spec/", "documentKey": key, "skills": skills });
        json::print(&json::result(SCHEMA, "created", fields, Vec::new()));
    } else {
        outln!("created spec/, document key {key}");
        if let Some(installed) = &installed {
            skill::report(installed);
        }
        if let Some(suggestion) = merge_driver::suggestion(&GitCommand::default(), root) {
            outln!("{suggestion}");
        }
    }
    Finished::success("created")
}
