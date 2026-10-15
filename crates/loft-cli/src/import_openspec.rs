//! `loft import openspec`: the main specs of an `OpenSpec` project converted into `spec/`, the
//! provenance sidecar beside it, the conversion findings, and the number of `V2-007` warnings
//! the new tree raises with the `spec/lint.yaml` entry that lowers them. Any refusal writes
//! nothing.

use std::path::Path;

use loftspec::codec;
use loftspec::convert::openspec::{Finding, SIDECAR, convert};
use loftspec::ids::DocumentId;
use loftspec::lint::{LintConfig, RuleId, Severity, count_body_rule};
use loftspec_store::FileStore;
use loftspec_store::openspec::read_project;
use serde_json::{Map, Value, json};

use crate::finish::Finished;
use crate::json;
use crate::out::{errln, outln};
use crate::stop::Stop;

const SCHEMA: &str = "loft-import-openspec/1";

/// The `spec/lint.yaml` entry that lowers `V2-007` to info.
const LOWER_V2_007: &str = "rules:\n  - rule: V2-007\n    severity: info\n";

pub fn run(root: &Path, dir: &Path, product: Option<&str>, as_json: bool) -> Finished {
    match import(root, dir, product) {
        Ok(imported) => {
            report(dir, &imported, as_json);
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
    let mut written: Vec<String> = files.keys().map(|path| format!("spec/{path}")).collect();
    written.push(SIDECAR.to_owned());
    let sidecar = conversion.sidecar.to_json();
    FileStore::open(root)
        .and_then(|store| store.import(files, (SIDECAR, sidecar.as_bytes())))
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

fn report(dir: &Path, imported: &Imported, as_json: bool) {
    let findings: Vec<Finding> = imported
        .findings
        .iter()
        .map(|finding| from_here(dir, finding))
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
            "findings": findings.iter().map(finding).collect::<Vec<_>>(),
            "advice": advice.into_iter().collect::<Vec<_>>(),
        });
        json::print(&json::result(SCHEMA, "imported", fields, Vec::new()));
    } else {
        for finding in &findings {
            outln!("{finding}");
        }
    }
    if let Some(advice) = advice(warnings) {
        errln!("{advice}");
    }
}

/// The finding with its file named by its path from the current directory.
fn from_here(dir: &Path, finding: &Finding) -> Finding {
    let mut finding = finding.clone();
    finding.file = finding
        .file
        .map(|file| dir.join(file).to_string_lossy().into_owned());
    finding
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

fn finding(finding: &Finding) -> Value {
    let mut fields = Map::new();
    fields.insert("rule".to_owned(), json!(finding.rule.as_str()));
    fields.insert("severity".to_owned(), json!(Severity::Info.as_str()));
    fields.insert("message".to_owned(), json!(finding.rule.message()));
    if let Some(file) = &finding.file {
        fields.insert("path".to_owned(), json!(file));
    }
    if let Some(line) = finding.line {
        fields.insert("line".to_owned(), json!(line));
    }
    if let Some(key) = finding.key {
        fields.insert("key".to_owned(), json!(key.to_string()));
    }
    if let Some(detail) = &finding.detail {
        fields.insert("detail".to_owned(), json!(detail));
    }
    Value::Object(fields)
}
