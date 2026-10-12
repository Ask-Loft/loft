//! `loft import v1` (F-37): a Loft v1 export converted into `spec/`, the sidecar beside it, and
//! the conversion findings printed as a lint report (AC-509). Any refusal writes nothing.

use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::{fs as stdfs, io};

use loftspec::codec::{self, Sites};
use loftspec::identity::LabelPrefix;
use loftspec::import::{ImportError, Imported, Reported, import};
use loftspec_store::fs;
use serde_json::{Map, Value, json};

use crate::json;

const SCHEMA: &str = "loft-import/1";

pub fn run(root: &Path, export: &Path, counters: Option<&Path>, as_json: bool) -> ExitCode {
    match prepare(export, counters).and_then(|imported| write(root, imported)) {
        Ok((imported, sites)) => {
            report(&imported, &sites, as_json);
            ExitCode::SUCCESS
        }
        Err(message) => {
            if as_json {
                json::print(&json!({
                    "schema": SCHEMA,
                    "written": [],
                    "findings": [],
                    "warnings": [],
                    "errors": [json::error("refused", &message, &[])],
                }));
            }
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// Reads both inputs and converts, naming the file a failure concerns (AC-519, AC-520, AC-668).
fn prepare(export: &Path, counters: Option<&Path>) -> Result<Imported, String> {
    let read = |path: &Path| -> Result<Vec<u8>, String> {
        stdfs::read(path).map_err(|error: io::Error| format!("{}: {error}", path.display()))
    };
    let text = String::from_utf8(read(export)?).map_err(|_| {
        format!(
            "{}: not a Loft v1 export: the bytes are not UTF-8",
            export.display()
        )
    })?;
    let counters_bytes = counters.map(read).transpose()?;
    import(&text, counters_bytes.as_deref()).map_err(|error| match (&error, counters) {
        (ImportError::Counters(_) | ImportError::CounterTooLow { .. }, Some(path)) => {
            format!("{}: {error}", path.display())
        }
        _ => format!("{}: {error}", export.display()),
    })
}

fn write(root: &Path, imported: Imported) -> Result<(Imported, Sites), String> {
    fs::write_import(root, &imported.files, &imported.sidecar)
        .map_err(|error| error.to_string())?;
    let (_, sites) = codec::read_sited(&imported.files).map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    Ok((imported, sites))
}

fn report(imported: &Imported, sites: &Sites, as_json: bool) {
    let findings: Vec<Reported> = imported.findings.iter().map(Reported::of).collect();
    let warning = warning(&imported.derived);
    if as_json {
        let mut written: Vec<String> = imported
            .files
            .keys()
            .map(|path| format!("spec/{path}"))
            .collect();
        written.push(fs::SIDECAR.to_owned());
        json::print(&json!({
            "schema": SCHEMA,
            "written": written,
            "findings": findings.iter().map(|f| finding(f, sites)).collect::<Vec<_>>(),
            "warnings": warning.iter().collect::<Vec<_>>(),
            "errors": [],
        }));
    } else {
        for reported in &findings {
            println!("{}", line(reported, sites));
        }
    }
    if let Some(warning) = warning {
        eprintln!("warning: {warning}");
    }
}

/// The counters warning (AC-516): the prefixes set one above the highest label seen.
fn warning(derived: &[LabelPrefix]) -> Option<String> {
    if derived.is_empty() {
        return None;
    }
    let names: Vec<&str> = derived.iter().map(|prefix| prefix.as_str()).collect();
    Some(format!(
        "the counters for {} are one above the highest label seen; the source may have \
         allocated higher values",
        names.join(", ")
    ))
}

fn line(reported: &Reported, sites: &Sites) -> String {
    let site = sites
        .get(reported.key)
        .map_or_else(|| reported.key.to_string(), ToString::to_string);
    let rule = reported.rule.map_or("import", |rule| rule.as_str());
    let mut text = format!("{site}: {} {rule}: {}", reported.severity, reported.message);
    if let Some(term) = &reported.term {
        let _ = write!(text, " `{term}`");
    }
    if let Some(line) = reported.body_line {
        let _ = write!(text, " (body line {line})");
    }
    text
}

fn finding(reported: &Reported, sites: &Sites) -> Value {
    let mut fields = sites.get(reported.key).map_or_else(Map::new, json::site);
    if let Some(rule) = reported.rule {
        fields.insert("rule".to_owned(), json!(rule.as_str()));
    }
    fields.insert("severity".to_owned(), json!(reported.severity.as_str()));
    fields.insert("message".to_owned(), json!(reported.message));
    fields.insert("key".to_owned(), json!(reported.key.to_string()));
    if let Some(term) = &reported.term {
        fields.insert("term".to_owned(), json!(term));
    }
    if let Some(line) = reported.body_line {
        fields.insert("bodyLine".to_owned(), json!(line));
    }
    Value::Object(fields)
}
