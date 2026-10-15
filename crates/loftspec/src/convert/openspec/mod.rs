//! Conversion from the main specs of an `OpenSpec` project, read as `OpenSpec` 1.14's archive
//! command reads them, into one Product, with a provenance sidecar and findings.

mod findings;
mod keys;
pub mod layout;
mod read;
pub mod sidecar;
pub(crate) mod statement;
mod tree;

use std::fmt::Write as _;

pub use self::findings::{Finding, Rule};
use self::layout::{CONFIG, SPEC_FILE, SPECS};
pub use self::sidecar::{OpenspecSidecar, Provenance, SIDECAR, SourceFile};
use self::tree::{Build, Capability};
use super::SidecarFile;
use crate::document::Document;
use crate::ids::DocumentId;

/// The files of one `openspec/` directory that the converter reads. The caller reads them; the
/// converter performs no input or output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Project {
    /// Each file named `spec.md` below `specs/`, by its path below `specs/` with `/` between
    /// segments, and its text.
    pub specs: Vec<(String, String)>,
    /// The text of a legacy `project.md`.
    pub project_md: Option<String>,
    /// The text of `config.yaml`.
    pub config_yaml: Option<String>,
    /// The ids of the active changes, under `changes/`.
    pub active_changes: Vec<String>,
    /// The number of archived changes, under `changes/archive/` and `archive/`.
    pub archived_changes: usize,
}

/// A converted document, its provenance and what the converter reported about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    pub document: Document,
    pub sidecar: OpenspecSidecar,
    pub findings: Vec<Finding>,
}

impl Conversion {
    /// The provenance sidecar, named, as the store writes it beside `spec/`.
    #[must_use]
    pub fn sidecar_file(&self) -> SidecarFile {
        SidecarFile {
            name: SIDECAR,
            bytes: self.sidecar.to_json().into_bytes(),
        }
    }
}

/// A project the converter refuses.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConvertError {
    /// The project holds no main spec; the active changes it holds, by id.
    #[error("{}", no_main_spec(.0))]
    NoMainSpec(Vec<String>),
}

fn no_main_spec(changes: &[String]) -> String {
    let mut message = "the project holds no main spec under `specs/`".to_owned();
    if !changes.is_empty() {
        let _ = write!(
            message,
            "; it holds {} active change{} ({}), which must be archived before it converts",
            changes.len(),
            if changes.len() == 1 { "" } else { "s" },
            changes.join(", ")
        );
    }
    message
}

/// Converts `project` into a document keyed `document` that holds one Product named `product`.
///
/// # Errors
///
/// The project holds no main spec.
pub fn convert(
    project: &Project,
    document: DocumentId,
    product: &str,
) -> Result<Conversion, ConvertError> {
    let capabilities = capabilities(project);
    if capabilities.is_empty() {
        return Err(ConvertError::NoMainSpec(project.active_changes.clone()));
    }
    let mut build = Build::new(document);
    let document = build.document(product, project.project_md.as_deref(), &capabilities);
    let (sidecar, conversion_findings) = build.finish();
    let mut findings = project_findings(project);
    findings.extend(conversion_findings);
    Ok(Conversion {
        document,
        sidecar,
        findings,
    })
}

/// The capabilities of `project` in the byte order of their ids: every `spec.md` below a
/// directory of `specs/`, none below a directory whose name begins with `.`.
fn capabilities(project: &Project) -> Vec<Capability<'_>> {
    let mut capabilities: Vec<Capability<'_>> = project
        .specs
        .iter()
        .filter_map(|(path, text)| {
            let id = path.strip_suffix(SPEC_FILE)?.strip_suffix('/')?;
            let hidden = id.split('/').any(|segment| segment.starts_with('.'));
            (!hidden && !id.is_empty()).then(|| Capability {
                id: id.to_owned(),
                file: format!("{SPECS}/{path}"),
                text,
            })
        })
        .collect();
    capabilities.sort_by(|a, b| a.id.as_bytes().cmp(b.id.as_bytes()));
    capabilities
}

/// The findings about the project as a whole: its changes and its configuration.
fn project_findings(project: &Project) -> Vec<Finding> {
    let mut findings = Vec::new();
    if !project.active_changes.is_empty() {
        let detail = format!(
            "{} ({})",
            project.active_changes.len(),
            project.active_changes.join(", ")
        );
        findings.push(Finding::about(Rule::ActiveChanges, detail));
    }
    if project.archived_changes > 0 {
        let detail = project.archived_changes.to_string();
        findings.push(Finding::about(Rule::ArchivedChanges, detail));
    }
    if let Some(config) = &project.config_yaml {
        for (index, line) in config.lines().enumerate() {
            let rule = if line.starts_with("context:") {
                Rule::ConfigContext
            } else if line.starts_with("store:") {
                Rule::Store
            } else {
                continue;
            };
            findings.push(Finding::at(rule, CONFIG.to_owned(), index + 1, None));
        }
    }
    findings
}
