//! The typed conflict record (F-34): what each side of a conflict holds, found from the
//! record that holds the conflict and the field path it names, so that a side is read and
//! written in the shape of that field (AC-860, RQ-220).

use super::error::{Located, Problem};
use super::record::{self, ConflictRec, Mode, Val};
use super::schema::{Item, Kind, Scalar, Shape, field};
use crate::yaml::{self, Out, Style, Value};

/// What each side of a conflict holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideShape {
    /// The key or value of the element a record follows on that side, or `first` (AC-861).
    Position,
    /// The record's key, or `null` on a side that deleted it.
    Record,
    /// The key of the record's parent on that side.
    Parent,
    /// A value in the shape of the field the path names.
    Field(Shape),
}

/// What each side of a conflict that `host` holds over `path` holds: `position`, `record`,
/// `parent`, or a field of `host`, dotted through the unkeyed records inside it
/// (`statement.shall`). None for a path that names no such field, or a field whose value holds
/// keyed records.
#[must_use]
pub fn side_shape(host: Kind, path: &str) -> Option<SideShape> {
    match path {
        field::POSITION => return Some(SideShape::Position),
        field::RECORD => return Some(SideShape::Record),
        field::PARENT => return Some(SideShape::Parent),
        _ => {}
    }
    let mut kind = host;
    let mut segments = path.split('.').peekable();
    while let Some(segment) = segments.next() {
        let (_, found) = kind.field(segment)?;
        if segments.peek().is_none() {
            return holds_no_keyed_record(found.shape).then_some(SideShape::Field(found.shape));
        }
        match found.shape {
            Shape::One(Item::Record(inner)) if !inner.hosts_conflicts() => kind = inner,
            _ => return None,
        }
    }
    None
}

/// Whether a value of `shape` holds no keyed record, so that a side never carries a key the
/// tree would count twice.
fn holds_no_keyed_record(shape: Shape) -> bool {
    match shape {
        Shape::One(Item::Record(kind)) | Shape::List(Item::Record(kind)) => !kind.is_keyed(),
        _ => true,
    }
}

/// Checks the `conflicts` list of a record of `host`.
pub(crate) fn check_conflicts(
    node: &yaml::Node,
    host: Kind,
    mode: Mode,
) -> Result<Vec<ConflictRec>, Located> {
    let items = match &node.value {
        Value::Sequence(items) => items.as_slice(),
        Value::Empty => &[],
        _ => return Err(located(node, Problem::Expected("a list"))),
    };
    items
        .iter()
        .map(|item| check_conflict(item, host, mode))
        .collect()
}

fn check_conflict(node: &yaml::Node, host: Kind, mode: Mode) -> Result<ConflictRec, Located> {
    let Value::Mapping(entries) = &node.value else {
        return Err(located(node, Problem::Expected("a mapping of fields")));
    };
    let path = entries
        .iter()
        .find(|entry| entry.key == field::FIELD)
        .ok_or_else(|| located(node, Problem::MissingField(field::FIELD)))?;
    let named = record::check_scalar(&path.value, Scalar::Identifier)?;
    let text = named.text().unwrap_or_default();
    let shape = side_shape(host, text).ok_or_else(|| {
        located(
            &path.value,
            Problem::Invalid {
                value: text.to_owned(),
                expected: "a field of the record that holds the conflict",
            },
        )
    })?;
    let mut conflict = ConflictRec::new(text, [None, None, None]);
    for entry in entries.iter().filter(|entry| entry.key != field::FIELD) {
        let Some(side) = conflict.side_mut(&entry.key) else {
            return Err(Located {
                position: entry.key_position,
                problem: Problem::UnknownField(entry.key.clone()),
            });
        };
        *side = Some(check_side(&entry.value, shape, mode)?);
    }
    Ok(conflict)
}

fn check_side(node: &yaml::Node, shape: SideShape, mode: Mode) -> Result<Val, Located> {
    match shape {
        SideShape::Position => record::check_scalar(node, Scalar::Identifier),
        SideShape::Record if is_null(node) => Ok(Val::Null {
            position: node.position,
        }),
        SideShape::Record | SideShape::Parent => record::check_scalar(node, Scalar::Key),
        SideShape::Field(shape) => record::check_shape(node, shape, mode),
    }
}

/// Whether a node is YAML's plain `null`; a quoted `"null"` is text.
fn is_null(node: &yaml::Node) -> bool {
    matches!(&node.value, Value::Scalar(scalar) if scalar.style == Style::Plain && scalar.text == "null")
}

fn located(node: &yaml::Node, problem: Problem) -> Located {
    Located {
        position: node.position,
        problem,
    }
}

/// The `conflicts` list of a record of `host` as the writer lays it out: each conflict's
/// fields in the Format's order, each side in the shape of the field it names (AC-857).
pub(crate) fn conflicts_out(conflicts: &[ConflictRec], host: Kind) -> Out {
    Out::Sequence(
        conflicts
            .iter()
            .map(|conflict| conflict_out(conflict, host))
            .collect(),
    )
}

fn conflict_out(conflict: &ConflictRec, host: Kind) -> Out {
    let shape =
        side_shape(host, &conflict.field).expect("a conflict the reader or the merge checked");
    let sides = conflict
        .sides()
        .into_iter()
        .filter_map(|(name, value)| value.map(|value| (name, side_out(value, shape))));
    let path = (field::FIELD, Out::Token(conflict.field.clone()));
    Out::Mapping(std::iter::once(path).chain(sides).collect())
}

fn side_out(value: &Val, shape: SideShape) -> Out {
    match (shape, value) {
        (_, Val::Null { .. }) => Out::Null,
        (SideShape::Field(shape), _) => record::value_out(value, shape),
        (SideShape::Position | SideShape::Record | SideShape::Parent, _) => {
            Out::Token(value.text().unwrap_or_default().to_owned())
        }
    }
}
