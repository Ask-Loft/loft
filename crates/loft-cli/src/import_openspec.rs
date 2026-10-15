//! `loft import openspec`: the main specs of an `OpenSpec` project converted into `spec/`, the
//! provenance sidecar beside it, the conversion findings, and the number of `V2-007` warnings
//! the new tree raises with the `spec/lint.yaml` entry that lowers them, and in text the next
//! steps: the skill, the merge driver and the adoption workflow. Any refusal writes nothing.

use std::path::Path;

use loftspec::codec;
use loftspec::convert::openspec::{Finding, convert};
use loftspec::ids::DocumentId;
use loftspec::lint::{LintConfig, RuleId, count_body_rule};
use loftspec_store::FileStore;
use loftspec_store::openspec::read_project;
use loftspec_store::repository::GitCommand;
use serde_json::json;

use crate::finish::Finished;
use crate::out::{errln, outln};
use crate::stop::Stop;
use crate::{json, merge_driver, skill};

const SCHEMA: &str = "loft-import-openspec/1";

/// The `spec/lint.yaml` entry that lowers `V2-007` to info.
const LOWER_V2_007: &str = "rules:\n  - rule: V2-007\n    severity: info\n";

/// The suggestion to adopt the import with the skill's workflow.
const ADOPT: &str = "next: have the coding agent adopt the import with the loft-spec skill's \
                     workflow \"Adopting an OpenSpec project\"";

pub fn run(root: &Path, dir: &Path, product: Option<&str>, as_json: bool) -> Finished {
    match import(root, dir, product) {
        Ok(imported) => {
            report(root, dir, imported, as_json);
            Finished::success("imported")
        }
        Err(message) => Stop::one("refused", message).exit(SCHEMA, as_json),
    }
}

/// What an import wrote and found.
struct Imported {
    written: Vec<String>,
    findings: Vec<Finding>,
    modal_warnings: usize,
}

fn import(root: &Path, dir: &Path, product: Option<&str>) -> Result<Imported, String> {
    let project = read_project(dir).map_err(|error| error.to_string())?;
    let product = match product {
        Some(name) => name.to_owned(),
        None => holder_name(dir)?,
    };
    let conversion = convert(&project, DocumentId::mint(), &product)
        .map_err(|error| format!("{}: {error}", dir.display()))?;
    let files = codec::write(&conversion.document);
    let sidecar = conversion.sidecar_file();
    let mut written: Vec<String> = files.keys().map(ToString::to_string).collect();
    written.push(sidecar.name.to_owned());
    FileStore::open(root)
        .and_then(|store| store.import(files, sidecar))
        .map_err(|error| error.to_string())?;
    // A new tree has no `spec/lint.yaml`, so `loft check` lints it under the defaults, which
    // make every `V2-007` finding a warning.
    let modal_warnings =
        count_body_rule(&conversion.document, &LintConfig::default(), RuleId::V2007);
    Ok(Imported {
        written,
        findings: conversion.findings,
        modal_warnings,
    })
}

/// The name of the directory that holds `dir`.
fn holder_name(dir: &Path) -> Result<String, String> {
    let absolute = dir
        .canonicalize()
        .map_err(|error| format!("{}: {error}", dir.display()))?;
    absolute
        .parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| {
            format!(
                "{}: no directory holds it to name the Product after; give `--product`",
                dir.display()
            )
        })
}

fn report(root: &Path, dir: &Path, imported: Imported, as_json: bool) {
    let findings: Vec<Finding> = imported
        .findings
        .into_iter()
        .map(|finding| finding.under(dir))
        .collect();
    let warnings = imported.modal_warnings;
    if as_json {
        let advice = (warnings > 0).then(|| {
            json!({
                "rule": RuleId::V2007.as_str(),
                "warnings": warnings,
                "lintEntry": LOWER_V2_007,
            })
        });
        let fields = json!({
            "written": imported.written,
            "findings": findings.iter().map(json::info_finding).collect::<Vec<_>>(),
            "advice": advice.into_iter().collect::<Vec<_>>(),
        });
        json::print(&json::result(SCHEMA, "imported", fields, Vec::new()));
    } else {
        for finding in &findings {
            outln!("{finding}");
        }
        for suggestion in suggestions(root) {
            outln!("{suggestion}");
        }
    }
    if let Some(advice) = advice(warnings) {
        errln!("{advice}");
    }
}

/// What to do after the import: install the skill where it is missing, register the merge
/// driver as `loft init` suggests, and adopt the import.
fn suggestions(root: &Path) -> Vec<String> {
    let skill = (!skill::installed(root)).then(|| {
        "next: run `loft skill install`, so the project's coding agents have the loft-spec skill"
            .to_owned()
    });
    let driver = merge_driver::suggestion(&GitCommand::default(), root);
    skill
        .into_iter()
        .chain(driver)
        .chain([ADOPT.to_owned()])
        .collect()
}

/// The report of the `V2-007` warnings the tree raises, if it raises any.
fn advice(warnings: usize) -> Option<String> {
    (warnings > 0).then(|| {
        format!(
            "note: `loft check` reports {warnings} V2-007 warning{}: OpenSpec states obligations \
             in scenario bodies, which become criterion bodies. To lower them to info while the \
             criteria are rewritten, add this entry to `spec/lint.yaml`:\n\n{LOWER_V2_007}",
            if warnings == 1 { "" } else { "s" }
        )
    })
}
