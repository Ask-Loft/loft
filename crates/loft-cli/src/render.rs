//! `loft render` (F-40): the Markdown projection and the canonical JSON export, written to the
//! paths given or to `docs/spec.md` and `docs/spec.json`. A read error, or an output path under
//! `spec/`, writes nothing (Reading the tree, RQ-171); a lint finding or a validation error does not
//! prevent a render (AC-549, AC-685).

use std::path::{Path, PathBuf};

use loftspec::codec;
use loftspec::render::render;
use loftspec_store::fs;
use serde_json::json;

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::stop::{self, Stop};

const SCHEMA: &str = "loft-render/1";
const DEFAULT_MARKDOWN: &str = "docs/spec.md";
const DEFAULT_JSON: &str = "docs/spec.json";

pub fn run(
    root: &Path,
    markdown: Option<PathBuf>,
    json_path: Option<PathBuf>,
    as_json: bool,
) -> Finished {
    match write(root, markdown, json_path) {
        Ok((written, unchanged)) => {
            let names = |paths: &[PathBuf]| -> Vec<String> {
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect()
            };
            if as_json {
                let fields = json!({ "written": names(&written), "unchanged": names(&unchanged) });
                json::print(&json::result(SCHEMA, "rendered", fields, Vec::new()));
            } else {
                for path in &written {
                    outln!("{}", path.display());
                }
            }
            Finished::success("rendered")
        }
        Err(stop) => stop.exit(SCHEMA, as_json),
    }
}

/// The paths written and the paths left unchanged.
type Outcome = (Vec<PathBuf>, Vec<PathBuf>);

fn write(
    root: &Path,
    markdown: Option<PathBuf>,
    json_path: Option<PathBuf>,
) -> Result<Outcome, Stop> {
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
        return Err(Stop::one("refused", message));
    }
    let files = stop::read_files(root)?;
    let document = codec::read(&files)?;
    let projections = render(&document).map_err(|errors| {
        let integrity = errors.integrity.iter().map(ToString::to_string);
        let cycles = errors.cycles.iter().map(ToString::to_string);
        Stop::all("integrity", integrity.chain(cycles).collect())
    })?;
    let outputs: Vec<(PathBuf, Vec<u8>)> = markdown
        .map(|path| (path, projections.markdown.into_bytes()))
        .into_iter()
        .chain(json_path.map(|path| (path, projections.json)))
        .collect();
    let written =
        fs::write_outputs(root, &outputs).map_err(|error| Stop::one("store", error.to_string()))?;
    let unchanged = outputs
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| !written.contains(path))
        .collect();
    Ok((written, unchanged))
}
