//! `loft init` (F-36): an empty document created in the current directory, with a fresh
//! document key, and the `loft-spec` skill beside it unless `--no-skill` is given, with the
//! plugin enabled unless `--no-plugin` is given. Refused, with
//! nothing written, where `spec/` exists already (RQ-129). It ends with the setup report: what
//! the project and the machine still need for the agents' hosts.

use std::path::Path;

use loftspec::codec;
use loftspec::document::Document;
use loftspec::ids::DocumentId;
use loftspec_store::FileStore;
use serde_json::json;

use crate::finish::Finished;
use crate::out::outln;
use crate::stop::Stop;
use crate::{json, setup, skill};

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-init/2";

/// Runs `loft init` in `root`, writing the skill where `skill` names how.
pub fn run(root: &Path, skill: Option<skill::Plugin>, as_json: bool) -> Finished {
    let document = Document::empty(DocumentId::mint());
    if let Err(error) =
        FileStore::open(root).and_then(|store| store.create(codec::write(&document)))
    {
        return Stop::from(error).exit(SCHEMA, as_json);
    }
    let installed = match skill.map(|plugin| skill::install(root, plugin)).transpose() {
        Ok(installed) => installed,
        Err(error) => {
            return json::stop(SCHEMA, vec![skill::error_json(&error)], &error, as_json);
        }
    };
    let skills = installed
        .as_ref()
        .map_or_else(Vec::new, |installed| installed.skills.clone());
    let key = document.id.to_string();
    let plugin = installed
        .as_ref()
        .is_some_and(|installed| installed.settings.is_some());
    let report = setup::report(root, plugin);
    if as_json {
        let fields = json!({
            "created": "spec/",
            "documentKey": key,
            "skills": skills,
            "setup": setup::json(&report),
        });
        json::print(&json::result(SCHEMA, "created", fields, Vec::new()));
    } else {
        outln!("created spec/, document key {key}");
        if let Some(installed) = &installed {
            skill::report(installed);
        }
        setup::print(&report);
    }
    Finished::success("created")
}
