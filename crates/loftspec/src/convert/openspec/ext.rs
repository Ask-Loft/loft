//! The `openspec` namespace of a node's `ext`: what the converter keeps there that the Format
//! has no field for, and how the `OpenSpec` projection reads it back. A Feature holds its
//! capability id and the archive placeholder its Purpose held; a Requirement holds its
//! requirement name, the modal word its statement was written with when that was `MUST`, and
//! its metadata lines as a list of `{label, value}`.

use std::collections::BTreeMap;

use crate::ext::{Ext, ExtValue};
use crate::names::Namespace;

const NAMESPACE: &str = "openspec";
const CAPABILITY: &str = "capability";
const PURPOSE: &str = "purpose";
const NAME: &str = "name";
const MODAL: &str = "modal";
const METADATA: &str = "metadata";
const LABEL: &str = "label";
const VALUE: &str = "value";

/// A Feature's `ext`: its capability id, and the placeholder its Purpose held, if any.
pub(super) fn feature(capability: &str, placeholder: Option<String>) -> Ext {
    openspec([
        (CAPABILITY, Some(text(capability))),
        (PURPOSE, placeholder.map(ExtValue::Text)),
    ])
}

/// A Requirement's `ext`: its name, the modal word its statement was written with when that
/// is not `SHALL`, and its metadata lines.
pub(super) fn requirement(name: &str, modal: Option<&str>, metadata: Vec<(String, String)>) -> Ext {
    let metadata = (!metadata.is_empty()).then(|| {
        ExtValue::List(
            metadata
                .into_iter()
                .map(|(label, value)| {
                    ExtValue::Map(BTreeMap::from([
                        (LABEL.to_owned(), ExtValue::Text(label)),
                        (VALUE.to_owned(), ExtValue::Text(value)),
                    ]))
                })
                .collect(),
        )
    });
    openspec([
        (NAME, Some(text(name))),
        (MODAL, modal.map(text)),
        (METADATA, metadata),
    ])
}

/// Extension data in the `openspec` namespace holding each entry that has a value.
fn openspec<const N: usize>(entries: [(&str, Option<ExtValue>); N]) -> Ext {
    let entries: BTreeMap<String, ExtValue> = entries
        .into_iter()
        .filter_map(|(key, value)| Some((key.to_owned(), value?)))
        .collect();
    Ext::from([(
        Namespace::parse(NAMESPACE).expect("a namespace"),
        ExtValue::Map(entries),
    )])
}

fn text(value: &str) -> ExtValue {
    ExtValue::Text(value.to_owned())
}

/// The text `field` holds in `ext.openspec`.
fn field<'a>(ext: &'a Ext, field: &str) -> Option<&'a str> {
    ext.get(NAMESPACE)?.get(field)?.text()
}

/// A Feature's capability id when it is a relative path: segments that are not empty, `.` or
/// `..` and hold no backslash, joined with `/`.
pub(crate) fn capability(ext: &Ext) -> Option<&str> {
    let path = field(ext, CAPABILITY)?;
    path.split('/')
        .all(|segment| !matches!(segment, "" | "." | "..") && !segment.contains('\\'))
        .then_some(path)
}

/// The placeholder a Feature's Purpose held.
pub(crate) fn purpose(ext: &Ext) -> Option<&str> {
    field(ext, PURPOSE)
}

/// A Requirement's requirement name.
pub(crate) fn name(ext: &Ext) -> Option<&str> {
    field(ext, NAME)
}

/// The modal word a Requirement's statement is written with: its `modal` when that is `SHALL`
/// or `MUST`, and `SHALL` otherwise.
pub(crate) fn modal(ext: &Ext) -> &str {
    match field(ext, MODAL) {
        Some(word @ ("SHALL" | "MUST")) => word,
        _ => "SHALL",
    }
}

/// The label and value of each metadata entry that holds both as text.
pub(crate) fn metadata(ext: &Ext) -> Vec<(&str, &str)> {
    let Some(ExtValue::List(entries)) = ext.get(NAMESPACE).and_then(|value| value.get(METADATA))
    else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let text = |field| entry.get(field).and_then(ExtValue::text);
            Some((text(LABEL)?, text(VALUE)?))
        })
        .collect()
}
