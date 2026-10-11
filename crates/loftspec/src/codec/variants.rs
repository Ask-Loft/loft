//! Records whose fields depend on one of their values: a statement's text or form (AC-130,
//! AC-131, AC-139, AC-140), a decision state's kind (AC-770..AC-775), a driver's kind (AC-776,
//! AC-777) and a quality band's kind (AC-765).

use super::error::{Located, Problem};
use super::record::Rec;
use super::schema::Kind;

/// The fields each value of a record's discriminating field takes, besides the fields every
/// record of its kind carries.
fn variant(kind: Kind, value: &str) -> &'static [&'static str] {
    match (kind, value) {
        (Kind::Statement, "ubiquitous") => &["system", "shall"],
        (Kind::Statement, "event_driven") => &["when", "system", "shall"],
        (Kind::Statement, "state_driven") => &["while", "system", "shall"],
        (Kind::Statement, "unwanted") => &["if", "system", "shall"],
        (Kind::Statement, "optional") => &["where", "system", "shall"],
        (Kind::State, "accepted") => &["selectedOption", "outcome"],
        (Kind::State, "rejected") => &["rationale"],
        (Kind::State, "superseded") => &["selectedOption", "outcome", "successor"],
        (Kind::State, "retired") => &["selectedOption", "outcome", "rationale"],
        (Kind::Driver, "local") => &["label", "statement"],
        (Kind::Driver, "subject") => &["subject"],
        (Kind::Band, "at_most") => &["upper"],
        (Kind::Band, "at_least") => &["lower"],
        (Kind::Band, "exactly") => &["value"],
        (Kind::Band, "between") => &["lower", "upper"],
        _ => &[],
    }
}

/// The field that selects a record's variant, and the fields every variant carries.
const fn discriminant(kind: Kind) -> Option<(&'static str, &'static [&'static str])> {
    match kind {
        Kind::Statement => Some(("form", &[])),
        Kind::State | Kind::Band => Some(("kind", &[])),
        Kind::Driver => Some(("kind", &["key"])),
        _ => None,
    }
}

/// Requires a record to hold exactly the fields its variant takes.
pub(super) fn check(record: &Rec) -> Result<(), Located> {
    let Some((selector, common)) = discriminant(record.kind) else {
        return Ok(());
    };
    let shape = |problem: String| Located {
        position: record.position,
        problem: Problem::Shape(problem),
    };
    if record.kind == Kind::Statement && record.get("text").is_some() {
        return match record.fields.len() {
            1 => Ok(()),
            _ => Err(shape(
                "a statement with `text` holds no other field".to_owned(),
            )),
        };
    }
    let Some(value) = record.text(selector) else {
        return Err(Located {
            position: record.position,
            problem: Problem::MissingField(if record.kind == Kind::Statement {
                "text"
            } else {
                selector
            }),
        });
    };
    let takes = variant(record.kind, value);
    for (name, _) in &record.fields {
        if *name != selector && !common.contains(name) && !takes.contains(name) {
            return Err(shape(format!("`{value}` takes no field `{name}`")));
        }
    }
    match takes.iter().find(|name| record.get(name).is_none()) {
        Some(missing) => Err(Located {
            position: record.position,
            problem: Problem::MissingField(missing),
        }),
        None => Ok(()),
    }
}
