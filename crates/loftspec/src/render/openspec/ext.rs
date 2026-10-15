//! The `openspec` namespace of a node's `ext`, as the renderer reads it.

use crate::ext::{Ext, ExtValue};
use crate::names::Namespace;

/// The value `field` holds in `ext.openspec`.
pub(super) fn openspec<'a>(ext: &'a Ext, field: &str) -> Option<&'a ExtValue> {
    ext.get(&Namespace::parse("openspec").expect("a namespace"))?
        .get(field)
}

/// The modal word a Requirement's statement is written with: its `ext.openspec.modal` when that
/// is `SHALL` or `MUST`, and `SHALL` otherwise.
pub(super) fn modal(ext: &Ext) -> &str {
    match openspec(ext, "modal").and_then(ExtValue::text) {
        Some(word @ ("SHALL" | "MUST")) => word,
        _ => "SHALL",
    }
}

/// The label and value of each entry of `ext.openspec.metadata` that holds both as text.
pub(super) fn metadata(ext: &Ext) -> Vec<(&str, &str)> {
    let Some(ExtValue::List(entries)) = openspec(ext, "metadata") else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let text = |field| entry.get(field).and_then(ExtValue::text);
            Some((text("label")?, text("value")?))
        })
        .collect()
}

/// A Feature's `ext.openspec.capability` when it is a relative path: segments that are not
/// empty, `.` or `..` and hold no backslash, joined with `/`.
pub(super) fn capability(ext: &Ext) -> Option<&str> {
    let path = openspec(ext, "capability").and_then(ExtValue::text)?;
    path.split('/')
        .all(|segment| !matches!(segment, "" | "." | "..") && !segment.contains('\\'))
        .then_some(path)
}
