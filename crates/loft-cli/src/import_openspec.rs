//! `loft import openspec`: the main specs of an `OpenSpec` project converted into `spec/`, the
//! provenance sidecar beside it, the conversion findings, and the number of `V2-007` warnings
//! the new tree raises with the `spec/lint.yaml` entry that lowers them. Any refusal writes
//! nothing.

use std::fs;
use std::path::Path;

use loftspec::check::check;
use loftspec::codec;
use loftspec::convert::openspec::{Finding, Project, SIDECAR, convert};
use loftspec::ids::DocumentId;
use loftspec::lint::{RuleId, Severity};
use loftspec_store::FileStore;
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
    let project = read_project(dir)?;
    let product = match product {
        Some(name) => name.to_owned(),
        None => holder_name(dir)?,
    };
    let conversion = convert(&project, DocumentId::mint(), &product)
        .map_err(|error| format!("{}: {error}", dir.display()))?;
    let files = codec::write(&conversion.document);
    let mut written: Vec<String> = files.keys().map(|path| format!("spec/{path}")).collect();
    written.push(SIDECAR.to_owned());
    let modal_warnings = check(&files)
        .findings()
        .iter()
        .filter(|finding| finding.rule == RuleId::V2007 && finding.severity == Severity::Warning)
        .count();
    let sidecar = conversion.sidecar.to_json();
    FileStore::open(root)
        .and_then(|store| store.import(files, (SIDECAR, sidecar.as_bytes())))
        .map_err(|error| error.to_string())?;
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

/// The files of the project that the converter takes.
fn read_project(dir: &Path) -> Result<Project, String> {
    fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let mut specs = Vec::new();
    walk(&dir.join("specs"), "", &mut specs)?;
    Ok(Project {
        specs,
        project_md: optional(&dir.join("project.md"))?,
        config_yaml: optional(&dir.join("config.yaml"))?,
        active_changes: directories(&dir.join("changes"))
            .into_iter()
            .filter(|name| name != "archive")
            .collect(),
        archived_changes: directories(&dir.join("changes").join("archive")).len()
            + directories(&dir.join("archive")).len(),
    })
}

/// Each `spec.md` below `dir`, by its path below the walk's start, never through a symbolic
/// link.
fn walk(dir: &Path, prefix: &str, specs: &mut Vec<(String, String)>) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if kind.is_dir() {
            walk(&entry.path(), &format!("{path}/"), specs)?;
        } else if kind.is_file() && name == "spec.md" {
            specs.push((path, text(&entry.path())?));
        }
    }
    Ok(())
}

fn optional(path: &Path) -> Result<Option<String>, String> {
    if path.is_file() {
        text(path).map(Some)
    } else {
        Ok(None)
    }
}

fn text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    String::from_utf8(bytes).map_err(|_| format!("{}: the bytes are not UTF-8", path.display()))
}

/// The names of the directories directly in `dir`, sorted; none when `dir` does not exist.
fn directories(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn report(dir: &Path, imported: &Imported, as_json: bool) {
    let findings: Vec<Finding> = imported
        .findings
        .iter()
        .map(|finding| from_here(dir, finding))
        .collect();
    let advice = advice(imported.modal_warnings);
    if as_json {
        let fields = json!({
            "written": imported.written,
            "findings": findings.iter().map(finding).collect::<Vec<_>>(),
            "advice": advice.iter().map(|_| json!({
                "rule": "V2-007",
                "warnings": imported.modal_warnings,
                "lintEntry": LOWER_V2_007,
            })).collect::<Vec<_>>(),
        });
        json::print(&json::result(SCHEMA, "imported", fields, Vec::new()));
    } else {
        for finding in &findings {
            outln!("{finding}");
        }
    }
    if let Some(advice) = advice {
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
    fields.insert("rule".to_owned(), json!(finding.rule.code()));
    fields.insert("severity".to_owned(), json!("info"));
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
