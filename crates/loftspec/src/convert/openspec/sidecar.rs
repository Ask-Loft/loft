//! The provenance sidecar: where each converted node came from, written beside `spec/` as
//! `openspec-sidecar.json`.

use serde::Serialize;

use crate::identity::Key;

/// The sidecar's file name beside `spec/`.
pub const SIDECAR: &str = "openspec-sidecar.json";

/// The sources the converter read and the source of each node it created, in document order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenspecSidecar {
    pub format: &'static str,
    pub sources: Vec<SourceFile>,
    pub nodes: Vec<Provenance>,
}

/// A file the converter read, by its path in the `openspec/` directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceFile {
    pub path: String,
    pub sha256: String,
}

/// Where one node came from: the file, the line of its heading, the capability it belongs to
/// and the name the source gives it, where each applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provenance {
    pub key: Key,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl OpenspecSidecar {
    pub(super) const fn new() -> Self {
        Self {
            format: "loft-openspec-sidecar/1",
            sources: Vec::new(),
            nodes: Vec::new(),
        }
    }

    /// The sidecar as JSON with a closing newline.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self).expect("the sidecar serialises");
        json.push('\n');
        json
    }
}
