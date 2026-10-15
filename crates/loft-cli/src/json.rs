//! The JSON forms of sites, errors and findings shared by the commands' results (RQ-155,
//! RQ-158): a site is `{path, line, column}`, or `{path, byte}` for bytes that are not UTF-8, and
//! an error or finding carries its first site's fields beside the list of all of them.

use std::fmt::Display;

use loftspec::check::Placed;
use loftspec::codec::{ReadError, Site};
use loftspec::lint::{Finding, Severity};
use loftspec::report::{InfoFinding, InfoRule};
use loftspec::yaml::Location;
use serde_json::{Map, Value, json};

use crate::finish::Finished;
use crate::out::outln;

/// A site's fields: the path, then the line and column where it has them (AC-593, AC-594).
pub fn site(site: &Site) -> Map<String, Value> {
    site_at(&site.path, site.location)
}

/// The fields of a site at `location` in the file `path`, which may lie outside `spec/`.
pub fn site_at(path: &impl Display, location: Option<Location>) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("path".to_owned(), json!(path.to_string()));
    match location {
        Some(Location::At(position)) => {
            fields.insert("line".to_owned(), json!(position.line));
            fields.insert("column".to_owned(), json!(position.column));
        }
        Some(Location::Byte(offset)) => {
            fields.insert("byte".to_owned(), json!(offset));
        }
        None => {}
    }
    fields
}

/// An error: its kind and message, its first site's fields, and every site.
pub fn error(kind: &str, message: &str, sites: &[Site]) -> Value {
    error_at(kind, message, sites.iter().map(site).collect())
}

/// An error at `sites`, each given as its fields.
pub fn error_at(kind: &str, message: &str, sites: Vec<Map<String, Value>>) -> Value {
    let mut fields = sites.first().cloned().unwrap_or_default();
    fields.insert("kind".to_owned(), json!(kind));
    fields.insert("message".to_owned(), json!(message));
    let sites = sites.into_iter().map(Value::Object).collect();
    fields.insert("sites".to_owned(), Value::Array(sites));
    Value::Object(fields)
}

/// A read error as JSON.
pub fn read_error(kind: &str, error: &ReadError) -> Value {
    self::error(kind, &error.problem.to_string(), &error.sites)
}

/// Read errors of the tree as JSON.
pub fn read_errors(errors: &[ReadError]) -> Vec<Value> {
    errors
        .iter()
        .map(|error| read_error("read", error))
        .collect()
}

/// Validation errors, each at the sites of the elements it names, as JSON.
pub fn validation_errors(errors: &[Placed]) -> Vec<Value> {
    errors
        .iter()
        .map(|placed| self::error("validation", &placed.error.to_string(), &placed.sites))
        .collect()
}

/// A finding: rule, severity, message, the term and body line where it has them, its first
/// subject's site, and every subject with its key and site.
pub fn finding(finding: &Finding) -> Value {
    let mut fields = finding
        .subjects
        .first()
        .and_then(|subject| subject.site.as_ref())
        .map(site)
        .unwrap_or_default();
    fields.insert("rule".to_owned(), json!(finding.rule.as_str()));
    fields.insert("severity".to_owned(), json!(finding.severity.as_str()));
    fields.insert("message".to_owned(), json!(finding.message));
    if let Some(term) = &finding.term {
        fields.insert("term".to_owned(), json!(term));
    }
    if let Some(line) = finding.body_line {
        fields.insert("bodyLine".to_owned(), json!(line));
    }
    let subjects = finding.subjects.iter().map(|subject| {
        let mut fields = subject.site.as_ref().map(site).unwrap_or_default();
        fields.insert("key".to_owned(), json!(subject.key.to_string()));
        Value::Object(fields)
    });
    fields.insert("subjects".to_owned(), Value::Array(subjects.collect()));
    Value::Object(fields)
}

/// The document of a command that ran to a result (F-45): its schema, its outcome, its result
/// `fields` and any errors the result carries.
pub fn result(schema: &str, outcome: &str, mut fields: Value, errors: Vec<Value>) -> Value {
    fields["schema"] = json!(schema);
    fields["outcome"] = json!(outcome);
    fields["errors"] = Value::Array(errors);
    fields
}

/// The document of a command that stopped on an error: its schema, the outcome `error` and the
/// errors, and nothing else (F-45).
pub fn failure(schema: &str, errors: Vec<Value>) -> Value {
    result(schema, "error", json!({}), errors)
}

/// Whether a finding of one of `severities` meets the `--fail-on` threshold, where one is given.
pub fn fails_on(
    threshold: Option<Severity>,
    severities: impl IntoIterator<Item = Severity>,
) -> bool {
    threshold.is_some_and(|threshold| severities.into_iter().any(|severity| severity >= threshold))
}

/// The errors a document carries.
pub fn errors_of(document: &Value) -> &[Value] {
    document["errors"].as_array().map_or(&[], Vec::as_slice)
}

/// Prints one JSON document on standard output.
pub fn print(document: &Value) {
    outln!(
        "{}",
        serde_json::to_string(document).expect("a JSON value serializes")
    );
}

/// Stops a command on an error: prints the failure document when JSON was asked for, and the
/// message on standard error, and exits 1.
pub fn stop(schema: &str, errors: Vec<Value>, message: &impl Display, as_json: bool) -> Finished {
    let lines = vec![message.to_string()];
    crate::stop::Stop { errors, lines }.exit(schema, as_json)
}

/// A conversion or render finding: its code, the severity info, its message, and the path,
/// line, key and detail it has.
pub fn info_finding<R: InfoRule>(finding: &InfoFinding<R>) -> Value {
    let mut fields = Map::new();
    fields.insert("rule".to_owned(), json!(finding.rule.code()));
    fields.insert("severity".to_owned(), json!(Severity::Info.as_str()));
    fields.insert("message".to_owned(), json!(finding.rule.message()));
    let optional = [
        ("path", finding.file.clone().map(Value::from)),
        ("line", finding.line.map(Value::from)),
        ("key", finding.key.map(|key| Value::from(key.to_string()))),
        ("detail", finding.detail.clone().map(Value::from)),
    ];
    for (name, value) in optional {
        if let Some(value) = value {
            fields.insert(name.to_owned(), value);
        }
    }
    Value::Object(fields)
}
