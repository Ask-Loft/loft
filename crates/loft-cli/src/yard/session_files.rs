//! The files each session keeps for a branch's Yard Log, its token totals, its activity and its
//! consultation, as the page reads them: each file as JSON, its live copy where there is one, in
//! the order of their session ids. A file that does not read is left out, since only its own
//! session's events change it and the next event writes it whole again.

use loftspec::yaml::{self, Node, Style, Value as Yaml};
use serde_json::{Map, Number, Value};
use yardlog::codec::session::writes_over;
use yardlog_store::YardStore;
use yardlog_store::session::SessionFile;

/// Every file of `kind` in the Yard Log of `store`, as JSON.
pub fn read(store: &YardStore, kind: SessionFile) -> Vec<Value> {
    entries(store, kind)
        .into_iter()
        .map(|(_, value)| value)
        .collect()
}

/// Whether session `session`'s `file` in the Yard Log of `store` is of a format this `loft` does
/// not write, and so is left as it is: a totals file of a newer totals format or one it does not
/// know, or a consultation file of a format other than its own.
pub fn foreign(store: &YardStore, file: SessionFile, session: &str) -> bool {
    store
        .session_file(file, session)
        .is_some_and(|bytes| !writes_over(file.folder(), &bytes))
}

/// A file's text as the page reads it, for the tests of what writes it.
#[cfg(test)]
pub fn as_read(text: &str) -> Value {
    json(
        &yaml::read(text.as_bytes())
            .ok()
            .flatten()
            .expect("the file reads"),
    )
}

/// Every file of `kind` in the Yard Log of `store`, by the session key that names it, as JSON.
pub fn entries(store: &YardStore, kind: SessionFile) -> Vec<(String, Value)> {
    store
        .session_files(kind)
        .into_iter()
        .filter_map(|(key, bytes)| Some((key, yaml::read(&bytes).ok().flatten()?)))
        .filter_map(|(key, node)| {
            let mut value = json(&node);
            // A file written before files named their host is a Claude Code session's.
            if let Some(file) = value.as_object_mut() {
                file.entry("host")
                    .or_insert_with(|| Value::String(yardlog::act::CLAUDE_CODE.to_owned()));
            }
            if let Some(adapter) = value.get("adapter") {
                let descriptor = yardlog::adapter::Descriptor::read(adapter).ok()?;
                value["adapter"] = descriptor.json();
            }
            Some((key, value))
        })
        .collect()
}

/// A node of the YAML subset as JSON: a whole number or `true` and `false` written plain as
/// themselves, every other scalar as text.
fn json(node: &Node) -> Value {
    match &node.value {
        Yaml::Mapping(entries) => Value::Object(
            entries
                .iter()
                .map(|entry| (entry.key.clone(), json(&entry.value)))
                .collect::<Map<_, _>>(),
        ),
        Yaml::Sequence(items) => Value::Array(items.iter().map(json).collect()),
        Yaml::Scalar(scalar) if scalar.style == Style::Plain => plain(&scalar.text),
        Yaml::Scalar(scalar) => Value::String(scalar.text.clone()),
        Yaml::Empty => Value::Null,
    }
}

fn plain(text: &str) -> Value {
    match text {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => text.parse::<u64>().map_or_else(
            |_| Value::String(text.to_owned()),
            |number| Value::Number(Number::from(number)),
        ),
    }
}
