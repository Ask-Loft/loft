//! Yard Server's answer to a batch of versions, `yardlog-answer/1`: for each version of the
//! batch, in the order sent, the version's key and its outcome, with a code beside an outcome of
//! `refused` or `later`. The sending settles each entry by its version's outcome; an outcome this
//! `loft` does not know settles nothing, and leaves its entry where it is.

use std::collections::BTreeMap;

use serde_json::Value;

/// The format Yard Server's answer to a batch names.
pub const FORMAT: &str = "yardlog-answer/1";

/// What Yard Server did with one version of a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It stored the version.
    Stored,
    /// It already held the same version.
    Held,
    /// It holds other contents under the version's key, and kept them.
    Conflict,
    /// It will never take the version, for the reason its code names.
    Refused(String),
    /// It cannot take the version now, but may once it is upgraded, for the reason its code names.
    Later(String),
    /// An outcome this `loft` does not know, or one without the code it needs.
    Unknown,
}

/// The outcome of each version the answer `body` names, by the version's key, where the body
/// reads as an answer of [`FORMAT`]; an entry without a version's key is passed over.
#[must_use]
pub fn read(body: &[u8]) -> Option<BTreeMap<String, Outcome>> {
    let answer: Value = serde_json::from_slice(body).ok()?;
    if answer.get("format")?.as_str()? != FORMAT {
        return None;
    }
    let outcomes = answer.get("outcomes")?.as_array()?;
    Some(
        outcomes
            .iter()
            .filter_map(|outcome| {
                let version = outcome.get("version")?.as_str()?;
                Some((version.to_owned(), outcome_of(outcome)))
            })
            .collect(),
    )
}

fn outcome_of(outcome: &Value) -> Outcome {
    let code = outcome
        .get("code")
        .and_then(Value::as_str)
        .map(str::to_owned);
    match (outcome.get("outcome").and_then(Value::as_str), code) {
        (Some("stored"), _) => Outcome::Stored,
        (Some("held"), _) => Outcome::Held,
        (Some("conflict"), _) => Outcome::Conflict,
        (Some("refused"), Some(code)) => Outcome::Refused(code),
        (Some("later"), Some(code)) => Outcome::Later(code),
        _ => Outcome::Unknown,
    }
}
