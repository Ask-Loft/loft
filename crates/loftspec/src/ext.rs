//! Extension data: what a tool keeps on a node that the Format does not define, under the tool's
//! own namespace of the node's `ext`. No rule of the Format reads it and no content hash
//! includes it; the state root holds it through the extensions root, so the lock pins it.

use std::collections::BTreeMap;

use serde::{Serialize, Serializer};
use serde_json::{Map, Value};

use crate::names::Namespace;

/// The largest integer an extension value holds: the largest that canonical JSON writes exactly.
pub const MAX_INTEGER: i64 = 9_007_199_254_740_991;

/// A node's extension data, by namespace; empty when the node has none.
pub type Ext = BTreeMap<Namespace, ExtValue>;

/// One extension value. A mapping's keys sort in UTF-8 byte order, as a `String`'s order does;
/// a list keeps its order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExtValue {
    Text(String),
    /// An integer within ±[`MAX_INTEGER`].
    Integer(i64),
    Boolean(bool),
    /// A list that is not empty.
    List(Vec<Self>),
    /// A mapping that is not empty.
    Map(BTreeMap<String, Self>),
}

impl ExtValue {
    /// The value as JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("an extension value serialises")
    }

    /// The entry `key` of a mapping; none for any other value.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Map(entries) => entries.get(key),
            _ => None,
        }
    }

    /// The text of a text value.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// The value as JSON writes it: a string, a number, a boolean, an array or an object.
impl Serialize for ExtValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Text(text) => serializer.serialize_str(text),
            Self::Integer(number) => serializer.serialize_i64(*number),
            Self::Boolean(flag) => serializer.serialize_bool(*flag),
            Self::List(items) => items.serialize(serializer),
            Self::Map(entries) => entries.serialize(serializer),
        }
    }
}

/// A node's extension data as a JSON object of its namespaces.
#[must_use]
pub fn to_json(ext: &Ext) -> Value {
    serde_json::to_value(ext).expect("extension data serialises")
}

/// Why JSON is not extension data the text form can hold, at the JSON Pointer (RFC 6901) of the
/// value refused: empty for the whole JSON.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{problem} at `{path}`")]
pub struct FromJsonError {
    pub path: String,
    pub problem: JsonProblem,
}

/// What the text form cannot hold of a JSON value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum JsonProblem {
    #[error("extension data is an object of namespaces")]
    NotAnObject,
    #[error("a namespace is a lowercase letter followed by lowercase letters and digits")]
    Namespace,
    #[error("an extension value is never null")]
    Null,
    #[error("an extension number is an integer, with no fraction and no exponent")]
    NotAnInteger,
    #[error("an extension integer lies from -9007199254740991 to 9007199254740991")]
    OutOfRange,
    #[error("an extension list or mapping holds an entry")]
    Empty,
    #[error("a mapping key is a letter followed by letters and digits")]
    Key,
}

/// Extension data from the JSON [`to_json`] writes it as: its exact inverse. JSON the text form
/// cannot hold is refused, so that what this gives writes and reads back as it is.
///
/// # Errors
///
/// JSON that is not an object of namespaces; `null`; a number with a fraction or an exponent, or
/// an integer beyond ±[`MAX_INTEGER`]; an empty array or object; or a mapping key that is not a
/// letter followed by letters and digits: each at its JSON Pointer.
pub fn from_json(json: &Value) -> Result<Ext, FromJsonError> {
    let refused = |path: String, problem| FromJsonError { path, problem };
    let Value::Object(namespaces) = json else {
        return Err(refused(String::new(), JsonProblem::NotAnObject));
    };
    namespaces
        .iter()
        .map(|(name, value)| {
            let path = pointer("", name);
            let namespace = Namespace::parse(name)
                .map_err(|_| refused(path.clone(), JsonProblem::Namespace))?;
            Ok((namespace, value_from(value, &path)?))
        })
        .collect()
}

fn value_from(json: &Value, path: &str) -> Result<ExtValue, FromJsonError> {
    let refused = |problem| FromJsonError {
        path: path.to_owned(),
        problem,
    };
    match json {
        Value::Null => Err(refused(JsonProblem::Null)),
        Value::Bool(flag) => Ok(ExtValue::Boolean(*flag)),
        Value::String(text) => Ok(ExtValue::Text(text.clone())),
        Value::Number(number) => {
            if !(number.is_i64() || number.is_u64()) {
                return Err(refused(JsonProblem::NotAnInteger));
            }
            number
                .as_i64()
                .filter(|integer| in_range(*integer))
                .map(ExtValue::Integer)
                .ok_or_else(|| refused(JsonProblem::OutOfRange))
        }
        Value::Array(items) if items.is_empty() => Err(refused(JsonProblem::Empty)),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(at, item)| value_from(item, &pointer(path, &at.to_string())))
            .collect::<Result<_, _>>()
            .map(ExtValue::List),
        Value::Object(entries) if entries.is_empty() => Err(refused(JsonProblem::Empty)),
        Value::Object(entries) => entries_from(entries, path).map(ExtValue::Map),
    }
}

fn entries_from(
    entries: &Map<String, Value>,
    path: &str,
) -> Result<BTreeMap<String, ExtValue>, FromJsonError> {
    entries
        .iter()
        .map(|(key, value)| {
            let path = pointer(path, key);
            if !is_field_name(key) {
                return Err(FromJsonError {
                    path,
                    problem: JsonProblem::Key,
                });
            }
            Ok((key.clone(), value_from(value, &path)?))
        })
        .collect()
}

/// Whether the text form holds `ext`, as [`from_json`] reads its JSON back: every integer within
/// ±[`MAX_INTEGER`], every list and mapping holding an entry, and every mapping key a field name.
pub(crate) fn held(ext: &Ext) -> bool {
    ext.values().all(value_held)
}

fn value_held(value: &ExtValue) -> bool {
    match value {
        ExtValue::Text(_) | ExtValue::Boolean(_) => true,
        ExtValue::Integer(integer) => in_range(*integer),
        ExtValue::List(items) => !items.is_empty() && items.iter().all(value_held),
        ExtValue::Map(entries) => {
            !entries.is_empty()
                && entries
                    .iter()
                    .all(|(key, value)| is_field_name(key) && value_held(value))
        }
    }
}

/// Whether a mapping key is one the text form writes as a field name, as the YAML reader reads
/// a key.
fn is_field_name(key: &str) -> bool {
    crate::yaml::is_key(key)
}

/// Whether an integer is within ±[`MAX_INTEGER`], the bound of canonical JSON that the text
/// form holds.
pub(crate) fn in_range(integer: i64) -> bool {
    (-MAX_INTEGER..=MAX_INTEGER).contains(&integer)
}

/// The JSON Pointer of `token` under `path`, with `~` and `/` escaped.
fn pointer(path: &str, token: &str) -> String {
    format!("{path}/{}", token.replace('~', "~0").replace('/', "~1"))
}
