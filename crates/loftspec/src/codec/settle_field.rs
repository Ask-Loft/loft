//! A conflict's field settled in the records the writer builds for a document: set to the value
//! a side holds or one its caller gives, checked against the shape of the field, or left out;
//! then read back as the strict reader reads it. The bindings in dispute are added to the
//! bindings the labels file holds. Where the value is a slug, the files it names
//! move, so the reader's checks of where each file lies wait for the document to be written
//! again.

use super::canon::canonicalize;
use super::conflict::{SideShape, side_shape};
use super::error::{Problem, ReadError};
use super::files::write_files;
use super::layout::{check_tree, parse_files};
use super::record::{Mode, Rec, Val};
use super::resolve::resolve;
use super::resolve_conflicts::conflict_owner;
use super::schema::{Item, Kind, Shape, field};
use super::unresolve::unresolve;
use crate::conflict::{ConflictOwner, ConflictValue};
use crate::document::Document;

/// Why a conflict's field could not be settled.
#[derive(Debug)]
pub(crate) enum FieldRefusal {
    /// No record of the document owns the conflict, or none holds the field its path names.
    NoField,
    /// The value does not fit the shape of the field.
    Shape,
    /// The settled records do not read.
    Unreadable(Vec<ReadError>),
}

/// The document with the field at `path` of the record that `owner` names set to `value`, or
/// left out for none, as the strict reader reads it.
pub(crate) fn settle_field(
    document: &Document,
    owner: ConflictOwner,
    path: &str,
    value: Option<&ConflictValue>,
) -> Result<Document, FieldRefusal> {
    let mut tree = unresolve(document);
    let record = tree
        .files
        .values_mut()
        .flat_map(|(_, content)| content.records_mut())
        .find_map(|record| owned_by(record, owner))
        .ok_or(FieldRefusal::NoField)?;
    let Some(SideShape::Field(shape)) = side_shape(record.kind, path) else {
        return Err(FieldRefusal::NoField);
    };
    let value = value
        .map(|value| field_value(value, shape).ok_or(FieldRefusal::Shape))
        .transpose()?;
    let value = if record.kind == Kind::Labels && path == field::BINDINGS {
        with_bindings(record, value)
    } else {
        value
    };
    if !record.set_at_path(path, value) {
        return Err(FieldRefusal::NoField);
    }
    canonicalize(&mut tree);
    let parsed =
        parse_files(&write_files(&tree), Mode::Strict).map_err(FieldRefusal::Unreadable)?;
    if let Err(errors) = check_tree(&parsed) {
        let errors: Vec<ReadError> = errors
            .into_iter()
            .filter(|error| !placement(&error.problem))
            .collect();
        if !errors.is_empty() {
            return Err(FieldRefusal::Unreadable(errors));
        }
    }
    resolve(&parsed).map_err(FieldRefusal::Unreadable)
}

/// The bindings of the labels record with those `given` added, each in place of a binding of the
/// same key: a conflict over `bindings` holds only the bindings in dispute, which the merged
/// list leaves out.
fn with_bindings(labels: &Rec, given: Option<Val>) -> Option<Val> {
    let Some(Val::List(given)) = given else {
        return labels.get(field::BINDINGS).cloned();
    };
    let keys: Vec<_> = given
        .iter()
        .filter_map(|each| match each {
            Val::Record(binding) => binding.key(),
            _ => None,
        })
        .collect();
    let kept = labels
        .records(field::BINDINGS)
        .filter(|binding| binding.key().is_none_or(|key| !keys.contains(&key)))
        .map(|binding| Val::Record(binding.clone()));
    Some(Val::List(kept.chain(given).collect()))
}

/// Whether a read error says only where a file lies, which writing the document again settles.
const fn placement(problem: &Problem) -> bool {
    matches!(
        problem,
        Problem::SlugMismatch { .. } | Problem::WrongDirectory(_)
    )
}

/// The record that owns the conflicts of `owner`: `record` or one inside it.
fn owned_by(record: &mut Rec, owner: ConflictOwner) -> Option<&mut Rec> {
    if conflict_owner(record) == Some(owner) {
        return Some(record);
    }
    record
        .children_mut()
        .find_map(|child| owned_by(child, owner))
}

/// A value in the shape of a field; none where it does not fit.
fn field_value(value: &ConflictValue, shape: Shape) -> Option<Val> {
    match (shape, value) {
        (Shape::List(item), ConflictValue::List(items)) => items
            .iter()
            .map(|each| item_value(each, item))
            .collect::<Option<_>>()
            .map(Val::List),
        (Shape::One(item), value) => item_value(value, item),
        (Shape::List(_), _) => None,
    }
}

fn item_value(value: &ConflictValue, item: Item) -> Option<Val> {
    match (item, value) {
        (Item::Scalar(_) | Item::NodeRef, ConflictValue::Scalar(text)) => Some(Val::scalar(text)),
        (Item::Record(kind), ConflictValue::Record(fields)) => record_value(kind, fields),
        (Item::NodeRef, ConflictValue::Record(fields)) => record_value(Kind::FormerLabel, fields),
        (Item::Ext, ConflictValue::Ext(ext)) => Some(Val::Ext {
            value: ext.clone(),
            position: crate::yaml::Position::new(1, 1),
        }),
        _ => None,
    }
}

fn record_value(kind: Kind, fields: &[(String, ConflictValue)]) -> Option<Val> {
    let mut record = Rec::new(kind);
    for (name, inner) in fields {
        let (_, field) = kind.field(name)?;
        record.set(field.name, field_value(inner, field.shape)?);
    }
    Some(Val::Record(record))
}
