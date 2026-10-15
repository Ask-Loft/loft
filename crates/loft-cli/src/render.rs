//! `loft render` (F-40): the Markdown projection and the canonical JSON export, written to the
//! paths given or to `docs/spec.md` and `docs/spec.json`, and the `OpenSpec` projection, written
//! to the directory `--openspec` names. A read error, an output path under `spec/`, or an
//! `OpenSpec` directory that is not empty without `--force` writes nothing (Reading the tree,
//! RQ-171); a lint finding, a validation error or an `OpenSpec` render finding does not prevent a
//! render (AC-549, AC-685).

use std::path::{Path, PathBuf};

use loftspec::codec;
use loftspec::index::IndexErrors;
use loftspec::render::openspec::{Finding, openspec};
use loftspec::render::render;
use loftspec_store::fs;
use loftspec_store::openspec::{WriteProjectionError, write_projection};
use serde_json::json;

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::stop::{self, Stop};

const SCHEMA: &str = "loft-render/2";
const DEFAULT_MARKDOWN: &str = "docs/spec.md";
const DEFAULT_JSON: &str = "docs/spec.json";

/// Where `loft render` writes.
pub struct Targets {
    pub markdown: Option<PathBuf>,
    pub json: Option<PathBuf>,
    pub openspec: Option<PathBuf>,
    /// Whether `--force` replaces the main specs of the `OpenSpec` directory.
    pub force: bool,
}

pub fn run(root: &Path, targets: Targets, as_json: bool) -> Finished {
    match write(root, targets) {
        Ok(rendered) => {
            let names = |paths: &[PathBuf]| -> Vec<String> {
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect()
            };
            if as_json {
                let fields = json!({
                    "written": names(&rendered.written),
                    "unchanged": names(&rendered.unchanged),
                    "removed": names(&rendered.removed),
                    "findings": rendered.findings.iter().map(json::info_finding).collect::<Vec<_>>(),
                });
                json::print(&json::result(SCHEMA, "rendered", fields, Vec::new()));
            } else {
                for path in &rendered.written {
                    outln!("{}", path.display());
                }
                for path in &rendered.removed {
                    outln!("removed {}", path.display());
                }
                for finding in &rendered.findings {
                    outln!("{finding}");
                }
            }
            Finished::success("rendered")
        }
        Err(stop) => stop.exit(SCHEMA, as_json),
    }
}

/// The paths written, left unchanged and removed, and the `OpenSpec` render findings with
/// their files named from the current directory.
#[derive(Default)]
struct Rendered {
    written: Vec<PathBuf>,
    unchanged: Vec<PathBuf>,
    removed: Vec<PathBuf>,
    findings: Vec<Finding>,
}

fn write(root: &Path, targets: Targets) -> Result<Rendered, Stop> {
    let (markdown, json_path) = match (targets.markdown, targets.json, &targets.openspec) {
        (None, None, None) => (Some(DEFAULT_MARKDOWN.into()), Some(DEFAULT_JSON.into())),
        (markdown, json_path, _) => (markdown, json_path),
    };
    let paths = [&markdown, &json_path, &targets.openspec];
    if let Some(inside) = paths
        .into_iter()
        .flatten()
        .find(|path| fs::is_under_spec(root, path))
    {
        let message = format!(
            "{}: a projection is never written under `spec/`",
            inside.display()
        );
        return Err(Stop::one("refused", message));
    }
    let files = stop::read_files(root)?;
    let document = codec::read(&files)?;
    let mut outputs: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    if markdown.is_some() || json_path.is_some() {
        let projections = render(&document).map_err(|errors| integrity(&errors))?;
        outputs.extend(markdown.map(|path| (path, projections.markdown.into_bytes())));
        outputs.extend(json_path.map(|path| (path, projections.json)));
    }
    let mut rendered = Rendered::default();
    if let Some(dir) = &targets.openspec {
        let mut projection = openspec(&document).map_err(|errors| integrity(&errors))?;
        rendered.findings = std::mem::take(&mut projection.findings)
            .into_iter()
            .map(|finding| finding.under(dir))
            .collect();
        let written =
            write_projection(&root.join(dir), projection, targets.force).map_err(|error| {
                match error {
                    WriteProjectionError::NotEmpty(_) | WriteProjectionError::Link(_) => {
                        Stop::one("refused", error.to_string())
                    }
                    WriteProjectionError::Store(error) => Stop::one("store", error.to_string()),
                }
            })?;
        let here = |paths: Vec<PathBuf>| paths.into_iter().map(|path| dir.join(path)).collect();
        rendered.written = here(written.written);
        rendered.unchanged = here(written.unchanged);
        rendered.removed = here(written.removed);
    }
    let written =
        fs::write_outputs(root, &outputs).map_err(|error| Stop::one("store", error.to_string()))?;
    rendered.written.extend(written.written);
    rendered.unchanged.extend(written.unchanged);
    Ok(rendered)
}

/// A document whose references do not resolve renders nothing.
fn integrity(errors: &IndexErrors) -> Stop {
    let integrity = errors.integrity.iter().map(ToString::to_string);
    let cycles = errors.cycles.iter().map(ToString::to_string);
    Stop::all("integrity", integrity.chain(cycles).collect())
}
