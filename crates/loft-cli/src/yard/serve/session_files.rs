//! The files each session keeps in a branch's Yard Log folder, its token totals and its
//! activity, as the page reads them: each file as JSON, in the order of their session ids. A
//! file that does not read is left out, since only its own session's events change it and the
//! next event writes it whole again.

use std::fs;
use std::path::Path;

use loftspec::yaml::{self, Node, Style, Value as Yaml};
use serde_json::{Map, Number, Value};
use yardlog_store::session::SessionFile;

/// Every file of `kind` in Yard Log's folder `folder`, as JSON.
pub fn read(folder: &Path, kind: SessionFile) -> Vec<Value> {
    let Ok(entries) = fs::read_dir(folder.join(kind.folder())) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yaml")
        })
        .collect();
    paths.sort();
    paths
        .iter()
        .filter_map(|path| fs::read(path).ok())
        .filter_map(|bytes| yaml::read(&bytes).ok().flatten())
        .map(|node| json(&node))
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
