//! Extension data: what a tool keeps on a node that the Format does not define, under the tool's
//! own namespace of the node's `ext`. No rule of the Format reads it and no content hash
//! includes it; the state root holds it through the extensions root, so the lock pins it.

use std::collections::BTreeMap;

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
        match self {
            Self::Text(text) => Value::String(text.clone()),
            Self::Integer(number) => Value::from(*number),
            Self::Boolean(flag) => Value::Bool(*flag),
            Self::List(items) => Value::Array(items.iter().map(Self::to_json).collect()),
            Self::Map(entries) => Value::Object(
                entries
                    .iter()
                    .map(|(key, value)| (key.clone(), value.to_json()))
                    .collect(),
            ),
        }
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

/// A node's extension data as a JSON object of its namespaces.
#[must_use]
pub fn to_json(ext: &Ext) -> Value {
    Value::Object(
        ext.iter()
            .map(|(namespace, value)| (namespace.to_string(), value.to_json()))
            .collect::<Map<_, _>>(),
    )
}
