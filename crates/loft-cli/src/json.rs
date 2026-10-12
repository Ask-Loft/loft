//! The JSON forms of sites, errors and findings shared by the commands' results (RQ-155,
//! RQ-158): a site is `{path, line, column}`, or `{path, byte}` for bytes that are not UTF-8, and
//! an error or finding carries its first site's fields beside the list of all of them.

use loftspec::check::Placed;
use loftspec::codec::{ReadError, Site};
use loftspec::lint::Finding;
use loftspec::yaml::Location;
use serde_json::{Map, Value, json};

/// A site's fields: the path, then the line and column where it has them (AC-593, AC-594).
pub fn site(site: &Site) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("path".to_owned(), json!(site.path.to_string()));
    match site.location {
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
    let mut fields = sites.first().map(site).unwrap_or_default();
    fields.insert("kind".to_owned(), json!(kind));
    fields.insert("message".to_owned(), json!(message));
    fields.insert(
        "sites".to_owned(),
        Value::Array(sites.iter().map(|s| Value::Object(site(s))).collect()),
    );
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

/// Prints one JSON document on standard output.
pub fn print(document: &Value) {
    println!(
        "{}",
        serde_json::to_string(document).expect("a JSON value serializes")
    );
}
