//! A report as data: the JSON document a `loft` view gives, kept in Yard Log's YAML as it was
//! computed.

use serde_json::{Map, Number, Value};

/// A JSON value, with its numbers kept as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Datum {
    Null,
    Bool(bool),
    /// A number as JSON writes it.
    Number(String),
    Text(String),
    List(Vec<Self>),
    /// Fields in the order written.
    Map(Vec<(String, Self)>),
}

impl Datum {
    /// The field `name` of a map.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Self> {
        match self {
            Self::Map(fields) => fields.iter().find(|(key, _)| key == name).map(|(_, v)| v),
            _ => None,
        }
    }
}

impl From<&Value> for Datum {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(flag) => Self::Bool(*flag),
            Value::Number(number) => Self::Number(number.to_string()),
            Value::String(text) => Self::Text(text.clone()),
            Value::Array(items) => Self::List(items.iter().map(Self::from).collect()),
            Value::Object(fields) => Self::Map(
                fields
                    .iter()
                    .map(|(key, value)| (key.clone(), Self::from(value)))
                    .collect(),
            ),
        }
    }
}

impl From<&Datum> for Value {
    fn from(datum: &Datum) -> Self {
        match datum {
            Datum::Null => Self::Null,
            Datum::Bool(flag) => Self::Bool(*flag),
            Datum::Number(text) => text
                .parse::<Number>()
                .map_or_else(|_| Self::String(text.clone()), Self::Number),
            Datum::Text(text) => Self::String(text.clone()),
            Datum::List(items) => Self::Array(items.iter().map(Self::from).collect()),
            Datum::Map(fields) => Self::Object(
                fields
                    .iter()
                    .map(|(key, value)| (key.clone(), Self::from(value)))
                    .collect::<Map<_, _>>(),
            ),
        }
    }
}
