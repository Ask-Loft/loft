//! A state's files as the wire carries them, each `path` and `text` by its path from the top of
//! the tree, and as the Format's reader takes them, by their paths under `spec/`.

use std::collections::BTreeMap;

use loftspec::codec::{Files, SpecPath};
use loftspec::publish::ReadTree;
use serde_json::{Value, json};

use crate::http::Response;
use crate::remote::problem::invalid;

pub(super) fn texts(files: &BTreeMap<String, String>) -> Vec<Value> {
    files
        .iter()
        .map(|(path, text)| json!({ "path": path, "text": text }))
        .collect()
}

/// The files of a state read from the wire: each `path` and `text`, by the path.
pub(super) fn wire_files(value: &Value) -> Option<BTreeMap<String, String>> {
    value
        .as_array()?
        .iter()
        .map(|file| {
            Some((
                file["path"].as_str()?.to_owned(),
                file["text"].as_str()?.to_owned(),
            ))
        })
        .collect()
}

/// The files as the Format's reader takes them, by their paths under `spec/`.
pub(super) fn spec_files(files: &BTreeMap<String, String>) -> Files {
    files
        .iter()
        .map(|(path, text)| {
            let under = path.strip_prefix("spec/").unwrap_or(path);
            (SpecPath::new(under), text.clone().into_bytes())
        })
        .collect()
}

/// The tree of `spec`, or `invalid_request` naming, from the top of the tree, each path a read
/// error names.
pub(super) fn read_tree(spec: &Files) -> Result<ReadTree<'_>, Response> {
    ReadTree::read(spec).map_err(|errors| {
        let mut paths: Vec<String> = errors
            .iter()
            .flat_map(|error| &error.sites)
            .map(|site| format!("spec/{}", site.path.as_str()))
            .collect();
        paths.sort();
        paths.dedup();
        invalid(&paths, None)
    })
}
