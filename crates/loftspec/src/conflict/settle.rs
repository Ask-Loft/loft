//! Settling one conflict of a document held in memory (Settling a conflict): by base's, ours'
//! or theirs' side, or, for a conflict over a field, by a value checked against the field's
//! shape. The document given is unchanged; the settled one, without the conflict record, is
//! written and read back, so that it reads or the settlement is refused.

use super::structure;
use super::{Conflict, ConflictId, ConflictOwner, ConflictValue, FieldPath};
use crate::codec::schema::field;
use crate::codec::{self, FieldRefusal, ReadError};
use crate::document::Document;
use crate::edit;
use crate::identity::Key;
use crate::merge::Input;

/// How a conflict is settled: by the value one of its sides holds, or by a value its caller
/// gives, which only a conflict over a field takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settlement {
    Base,
    Ours,
    Theirs,
    Value(ConflictValue),
}

/// Why a conflict could not be settled; the document is unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettleError {
    #[error("the document holds no conflict {0}")]
    Unknown(ConflictId),
    #[error("a conflict over `{0}` takes a side, not a value")]
    SideOnly(FieldPath),
    #[error("the value does not fit the shape of `{0}`")]
    Shape(FieldPath),
    #[error("{side} holds nothing for the conflict over `{field}`")]
    NoSide { side: Input, field: FieldPath },
    #[error("the document holds no record {0} where the settlement names it")]
    NoRecord(Key),
    #[error("{0} cannot be settled so")]
    Unsettleable(ConflictOwner),
    #[error("{0}")]
    Edit(edit::Refusal),
    #[error("the settled document does not read")]
    Unreadable(Vec<ReadError>),
}

/// The document with the conflict `id` settled by `settlement`, without its conflict record:
/// a field takes the side's value, or the value given, or is left out where the side lacks it;
/// a record whose existence is in conflict is kept or deleted, with its bindings departed; and a
/// record whose parent or position is in conflict moves where the side puts it. Every other
/// conflict record stays as it was.
///
/// # Errors
///
/// No conflict has the identifier; a value is given for a conflict over `record`, `parent` or
/// `position`, or does not fit the field's shape; the side holds nothing for such a conflict, or
/// names a record the document does not hold there; the edit the settlement makes is refused;
/// or the settled document does not read.
pub fn settle(
    document: &Document,
    id: ConflictId,
    settlement: &Settlement,
) -> Result<Document, SettleError> {
    let at = (document.conflicts.iter())
        .position(|each| each.id() == id)
        .ok_or(SettleError::Unknown(id))?;
    let conflict = &document.conflicts[at];
    let structural = matches!(
        conflict.field.as_str(),
        field::RECORD | field::PARENT | field::POSITION
    );
    let (input, side) = match settlement {
        Settlement::Value(_) if structural => {
            return Err(SettleError::SideOnly(conflict.field.clone()));
        }
        Settlement::Value(value) => {
            return codec::settle_field(document, conflict, Some(value))
                .map_err(|refusal| refused(conflict, refusal));
        }
        Settlement::Base => (Input::Base, conflict.base.as_ref()),
        Settlement::Ours => (Input::Ours, conflict.ours.as_ref()),
        Settlement::Theirs => (Input::Theirs, conflict.theirs.as_ref()),
    };
    if !structural {
        return codec::settle_field(document, conflict, side)
            .map_err(|refusal| refused(conflict, refusal));
    }
    let side = side.ok_or_else(|| SettleError::NoSide {
        side: input,
        field: conflict.field.clone(),
    })?;
    let mut settled = document.clone();
    settled.conflicts.remove(at);
    structure::settle(&mut settled, conflict, side)?;
    let (_, read) = codec::write_and_read(&settled);
    read.map(|(document, _)| document)
        .map_err(SettleError::Unreadable)
}

fn refused(conflict: &Conflict, refusal: FieldRefusal) -> SettleError {
    match refusal {
        FieldRefusal::NoField => SettleError::Unsettleable(conflict.owner),
        FieldRefusal::Shape => SettleError::Shape(conflict.field.clone()),
        FieldRefusal::Unreadable(errors) => SettleError::Unreadable(errors),
    }
}
