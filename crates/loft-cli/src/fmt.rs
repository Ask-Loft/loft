//! `loft fmt` (F-39): the tree rewritten in canonical layout with missing keys minted, or with
//! `--check` the drift reported and nothing written (RQ-140, RQ-142).

use std::path::Path;
use std::process::ExitCode;

use loftspec::fmt::format;
use loftspec_store::FileStore;
use serde_json::json;

use crate::json;
use crate::stop::Stop;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-fmt/1";

/// What a run did: the files that change, the keys minted, and whether it wrote them.
struct Outcome {
    changed: Vec<String>,
    minted: usize,
    written: bool,
}

impl Outcome {
    /// The outcome's name: `formatted` when files were rewritten, `canonical` when none would
    /// change, `drift` when `--check` finds one that would.
    const fn name(&self) -> &'static str {
        if self.written {
            "formatted"
        } else if self.changed.is_empty() {
            "canonical"
        } else {
            "drift"
        }
    }
}

pub fn run(root: &Path, check: bool, as_json: bool) -> ExitCode {
    match attempt(root, check) {
        Ok(outcome) => {
            let name = outcome.name();
            if as_json {
                let fields = json!({
                    "changed": outcome.changed,
                    "minted": outcome.minted,
                    "written": outcome.written,
                });
                json::print(&json::result(SCHEMA, name, fields, Vec::new()));
            } else {
                for path in &outcome.changed {
                    println!("{path}");
                }
            }
            if name == "drift" {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(stop) => stop.exit(SCHEMA, as_json),
    }
}

fn attempt(root: &Path, check: bool) -> Result<Outcome, Stop> {
    let store = FileStore::open(root)?;
    let formatted = format(&store.read_files()?)?;
    // A key lost by accident drops a binding too, so every drop is told.
    for (label, key) in &formatted.dropped {
        eprintln!(
            "spec/labels.yaml: the binding of {label} ({key}) names no node or decision, so \
             `loft fmt` drops it"
        );
    }
    let changed = formatted.changed.keys().map(ToString::to_string).collect();
    let written = !check && !formatted.changed.is_empty();
    if written {
        store.write_files(formatted.changed)?;
    }
    Ok(Outcome {
        changed,
        minted: formatted.minted,
        written,
    })
}
