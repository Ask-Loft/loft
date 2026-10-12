//! `loft render` (F-40): the Markdown projection and the canonical JSON export, written to the
//! paths given or to `docs/spec.md` and `docs/spec.json`. A read error, or an output path under
//! `spec/`, writes nothing (Reading the tree, RQ-171); a lint finding or a validation error does not
//! prevent a render (AC-549, AC-685).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use loftspec::codec;
use loftspec::render::render;
use loftspec_store::{FileStore, fs};
use serde_json::{Value, json};

use crate::json;

const SCHEMA: &str = "loft-render/1";
const DEFAULT_MARKDOWN: &str = "docs/spec.md";
const DEFAULT_JSON: &str = "docs/spec.json";

/// A failure that writes nothing: its errors as JSON and as text lines.
struct Refusal {
    errors: Vec<(Value, String)>,
}

impl Refusal {
    fn one(kind: &'static str, message: String) -> Self {
        let error = json::error(kind, &message, &[]);
        Self {
            errors: vec![(error, message)],
        }
    }
}

pub fn run(
    root: &Path,
    markdown: Option<PathBuf>,
    json_path: Option<PathBuf>,
    as_json: bool,
) -> ExitCode {
    match write(root, markdown, json_path) {
        Ok((written, unchanged)) => {
            let names = |paths: &[PathBuf]| -> Vec<String> {
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect()
            };
            if as_json {
                json::print(&json!({
                    "schema": SCHEMA,
                    "written": names(&written),
                    "unchanged": names(&unchanged),
                    "errors": [],
                }));
            } else {
                for path in &written {
                    println!("{}", path.display());
                }
            }
            ExitCode::SUCCESS
        }
        Err(refusal) => {
            if as_json {
                let errors: Vec<Value> = refusal.errors.iter().map(|(e, _)| e.clone()).collect();
                json::print(&json!({
                    "schema": SCHEMA,
                    "written": [],
                    "unchanged": [],
                    "errors": errors,
                }));
            }
            for (_, line) in &refusal.errors {
                eprintln!("{line}");
            }
            ExitCode::FAILURE
        }
    }
}

/// The paths written and the paths left unchanged.
type Outcome = (Vec<PathBuf>, Vec<PathBuf>);

fn write(
    root: &Path,
    markdown: Option<PathBuf>,
    json_path: Option<PathBuf>,
) -> Result<Outcome, Refusal> {
    let (markdown, json_path) = match (markdown, json_path) {
        (None, None) => (Some(DEFAULT_MARKDOWN.into()), Some(DEFAULT_JSON.into())),
        given => given,
    };
    let targets: Vec<&PathBuf> = markdown.iter().chain(json_path.iter()).collect();
    if let Some(inside) = targets.iter().find(|path| fs::is_under_spec(root, path)) {
        let message = format!(
            "{}: a projection is never written under `spec/`",
            inside.display()
        );
        return Err(Refusal::one("refused", message));
    }
    let files = FileStore::open(root)
        .and_then(|store| store.read_files())
        .map_err(|error| Refusal::one("store", error.to_string()))?;
    let document = codec::read(&files).map_err(|errors| Refusal {
        errors: errors
            .iter()
            .map(|error| (json::read_error("read", error), error.to_string()))
            .collect(),
    })?;
    let projections = render(&document).map_err(|errors| Refusal {
        errors: errors
            .integrity
            .iter()
            .map(ToString::to_string)
            .chain(errors.cycles.iter().map(ToString::to_string))
            .map(|message| (json::error("integrity", &message, &[]), message))
            .collect(),
    })?;
    let outputs: Vec<(PathBuf, Vec<u8>)> = markdown
        .map(|path| (path, projections.markdown.into_bytes()))
        .into_iter()
        .chain(json_path.map(|path| (path, projections.json)))
        .collect();
    let written = fs::write_outputs(root, &outputs)
        .map_err(|error| Refusal::one("store", error.to_string()))?;
    let unchanged = outputs
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| !written.contains(path))
        .collect();
    Ok((written, unchanged))
}
