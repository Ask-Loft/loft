//! A node record's `ext` in the text form: a mapping of namespaces to values. Text is written in
//! double quotes or as a literal block; a plain scalar is an integer or `true` or `false`.

use std::collections::BTreeMap;

use super::error::{Located, Problem};
use super::record::Val;
use crate::ext::{Ext, ExtValue, in_range};
use crate::names::Namespace;
use crate::yaml::{self, Out, Style, Value};

const PLAIN: &str = "an integer, `true`, `false`, or text in double quotes";
const INTEGER: &str = "an integer from -9007199254740991 to 9007199254740991";

/// Checks a node as a record's `ext`.
pub(super) fn check(node: &yaml::Node) -> Result<Val, Located> {
    let Value::Mapping(entries) = &node.value else {
        return Err(located(node, Problem::Expected("a mapping of namespaces")));
    };
    let mut ext = Ext::new();
    for entry in entries {
        let namespace = Namespace::parse(&entry.key).map_err(|_| Located {
            position: entry.key_position,
            problem: Problem::Invalid {
                value: entry.key.clone(),
                expected: "an extension namespace",
            },
        })?;
        ext.insert(namespace, value(&entry.value)?);
    }
    Ok(Val::Ext {
        value: ext,
        position: node.position,
    })
}

fn value(node: &yaml::Node) -> Result<ExtValue, Located> {
    match &node.value {
        Value::Mapping(entries) => entries
            .iter()
            .map(|entry| Ok((entry.key.clone(), value(&entry.value)?)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(ExtValue::Map),
        Value::Sequence(items) => items
            .iter()
            .map(value)
            .collect::<Result<_, _>>()
            .map(ExtValue::List),
        Value::Scalar(scalar) if scalar.style != Style::Plain => {
            Ok(ExtValue::Text(scalar.text.clone()))
        }
        Value::Scalar(scalar) => plain(node, &scalar.text),
        Value::Empty => Err(located(node, Problem::Expected("a value"))),
    }
}

/// A plain scalar: `true`, `false`, or an integer in range written without leading zeros.
fn plain(node: &yaml::Node, text: &str) -> Result<ExtValue, Located> {
    let invalid = |expected| {
        located(
            node,
            Problem::Invalid {
                value: text.to_owned(),
                expected,
            },
        )
    };
    match text {
        "true" => return Ok(ExtValue::Boolean(true)),
        "false" => return Ok(ExtValue::Boolean(false)),
        _ => {}
    }
    let digits = text.strip_prefix('-').unwrap_or(text);
    let decimal = !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && text != "-0";
    if !decimal {
        return Err(invalid(PLAIN));
    }
    text.parse::<i64>()
        .ok()
        .filter(|number| in_range(*number))
        .map(ExtValue::Integer)
        .ok_or_else(|| invalid(INTEGER))
}

const fn located(node: &yaml::Node, problem: Problem) -> Located {
    Located {
        position: node.position,
        problem,
    }
}

/// A node's `ext` as the writer lays it out.
pub(super) fn ext_out(ext: &Ext) -> Out {
    Out::Map(
        ext.iter()
            .map(|(namespace, value)| (namespace.to_string(), value_out(value)))
            .collect(),
    )
}

fn value_out(value: &ExtValue) -> Out {
    match value {
        ExtValue::Text(text) => Out::Text(text.clone()),
        ExtValue::Integer(number) => Out::Plain(number.to_string()),
        ExtValue::Boolean(flag) => Out::Plain(flag.to_string()),
        ExtValue::List(items) => Out::Sequence(items.iter().map(value_out).collect()),
        ExtValue::Map(entries) => Out::Map(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), value_out(value)))
                .collect(),
        ),
    }
}
