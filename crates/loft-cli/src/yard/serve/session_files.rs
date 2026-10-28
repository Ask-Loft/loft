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
    entries(folder, kind)
        .into_iter()
        .map(|(_, value)| value)
        .collect()
}

/// Every file of `kind` in Yard Log's folder `folder`, by the session key that names it, as
/// JSON.
pub fn entries(folder: &Path, kind: SessionFile) -> Vec<(String, Value)> {
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
        .filter_map(|path| {
            let key = path.file_stem()?.to_str()?.to_owned();
            let node = yaml::read(&fs::read(path).ok()?).ok().flatten()?;
            Some((key, node))
        })
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
pub(super) fn json(node: &Node) -> Value {
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
