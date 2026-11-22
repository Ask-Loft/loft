//! References into another document as the reader checks them: a record of the other document's
//! handle and a node's key, which only `provides`, `requires`, `mentions` and a subject's `node`
//! may hold, or, while formatting, of the handle and the node's label or name.

use super::Mode;
use super::error::{Located, Problem};
use super::schema::{Field, Kind, field};
use crate::yaml::{self, Position, Value};

/// Where a reference field that may not hold a reference into another document holds one.
pub(super) fn barred_crossing(node: &yaml::Node, field: &Field) -> Option<Position> {
    if field.shape.item().target().is_none() || field::CROSSING.contains(&field.name) {
        return None;
    }
    match &node.value {
        Value::Sequence(items) => items.iter().find(|item| foreign(item)),
        _ => foreign(node).then_some(node),
    }
    .map(|found| found.position)
}

/// The kind of a reference written as a record whose fields are named `fields`: a reference
/// into another document where it names one, otherwise a former label.
pub(super) fn reference_kind<'n>(mut fields: impl Iterator<Item = &'n str>) -> Kind {
    if fields.any(|name| name == field::DOCUMENT) {
        Kind::ForeignRef
    } else {
        Kind::FormerLabel
    }
}

/// Whether a reference is written as a record naming another document, or a node's key without
/// a name, as only a reference into another document is.
pub(super) fn foreign(node: &yaml::Node) -> bool {
    let Value::Mapping(entries) = &node.value else {
        return false;
    };
    let has = |name: &str| entries.iter().any(|entry| entry.key == name);
    has(field::DOCUMENT) || (has(field::KEY) && !has(field::NAME))
}

/// The kind of record a reference into another document is read as: by key, or, while
/// formatting, by label or by name, which a strict read refuses as a reference by name.
pub(super) fn foreign_kind(node: &yaml::Node, mode: Mode) -> Result<Kind, Located> {
    let Value::Mapping(entries) = &node.value else {
        return Ok(Kind::ForeignRef);
    };
    let named = entries
        .iter()
        .any(|entry| entry.key == field::LABEL || entry.key == field::NAME);
    match (named, mode) {
        (false, _) => Ok(Kind::ForeignRef),
        (true, Mode::Formatting) => Ok(Kind::ForeignByName),
        (true, Mode::Strict) => Err(Located {
            position: node.position,
            problem: Problem::ByName,
        }),
    }
}
