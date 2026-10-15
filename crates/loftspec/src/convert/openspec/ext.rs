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

/// A Feature's capability id when it is a relative path: segments that are not empty, do not
/// begin with `.` and hold no backslash, joined with `/`.
pub(crate) fn capability(ext: &Ext) -> Option<&str> {
    field(ext, CAPABILITY).filter(|path| safe(path))
}

fn safe(path: &str) -> bool {
    path.split('/')
        .all(|segment| !segment.is_empty() && !segment.starts_with('.') && !segment.contains('\\'))
}

/// The placeholder a Feature's Purpose held.
pub(crate) fn purpose(ext: &Ext) -> Option<&str> {
    field(ext, PURPOSE)
}

/// A Requirement's requirement name.
pub(crate) fn name(ext: &Ext) -> Option<&str> {
    field(ext, NAME)
}

/// The modal words a Requirement's statement may be written with.
const MODALS: [&str; 3] = ["SHALL", "MUST", "shall"];

/// The modal word a Requirement's statement is written with: its `modal` when that is `SHALL`,
/// `MUST` or `shall`, and `SHALL` otherwise.
pub(crate) fn modal(ext: &Ext) -> &str {
    field(ext, MODAL)
        .filter(|word| MODALS.contains(word))
        .unwrap_or(MODALS[0])
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

/// The fields of a Feature's `ext.openspec` that hold a value the renderer cannot read, each as
/// `ext.openspec.<field>`.
pub(crate) fn unreadable_feature(ext: &Ext) -> Vec<String> {
    unreadable(ext, |entries, bad| {
        bad(
            CAPABILITY,
            entries
                .get(CAPABILITY)
                .is_some_and(|value| value.text().is_none_or(|path| !safe(path))),
        );
        bad(
            PURPOSE,
            entries
                .get(PURPOSE)
                .is_some_and(|value| value.text().is_none()),
        );
    })
}

/// The fields of a Requirement's `ext.openspec` that hold a value the renderer cannot read, each
/// as `ext.openspec.<field>`.
pub(crate) fn unreadable_requirement(ext: &Ext) -> Vec<String> {
    unreadable(ext, |entries, bad| {
        bad(
            NAME,
            entries
                .get(NAME)
                .is_some_and(|value| value.text().is_none()),
        );
        let modal = entries.get(MODAL);
        bad(
            MODAL,
            modal.is_some_and(|value| value.text().is_none_or(|word| !MODALS.contains(&word))),
        );
        bad(
            METADATA,
            entries
                .get(METADATA)
                .is_some_and(|value| !readable_metadata(value)),
        );
    })
}

/// The fields `check` marks bad, or the namespace itself when it is not a mapping.
fn unreadable(
    ext: &Ext,
    check: impl FnOnce(&BTreeMap<String, ExtValue>, &mut dyn FnMut(&str, bool)),
) -> Vec<String> {
    match ext.get(NAMESPACE) {
        None => Vec::new(),
        Some(ExtValue::Map(entries)) => {
            let mut fields = Vec::new();
            check(entries, &mut |field, bad| {
                if bad {
                    fields.push(format!("ext.{NAMESPACE}.{field}"));
                }
            });
            fields
        }
        Some(_) => vec![format!("ext.{NAMESPACE}")],
    }
}

/// Whether a metadata value is a list whose every entry holds a text `label` and `value`.
fn readable_metadata(value: &ExtValue) -> bool {
    let ExtValue::List(entries) = value else {
        return false;
    };
    entries.iter().all(|entry| {
        [LABEL, VALUE]
            .iter()
            .all(|field| entry.get(field).and_then(ExtValue::text).is_some())
    })
}
