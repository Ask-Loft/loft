//! A batch of versions, `yardlog-batch/1`: what one request carries to Yard Server. Each version
//! goes as the JSON form of its record in its stage's file, written from the same tree of fields
//! as the YAML and in the same order, so the file and the batch cannot drift apart; beside it,
//! not inside it, go the keys that place it and its block's kind and id. A `loft` block's report,
//! which the file holds as JSON text, goes as the JSON it holds.
//!
//! The records are written as JSON text rather than as `serde_json` values, whose objects keep
//! no order of their members.

use std::fmt::Write as _;

use loft_common::yaml::Out;

use crate::body::Body;
use crate::codec::write_version;
use crate::keys::{StageKey, YardLogKey};
use crate::log::{Block, Version};

/// The format a batch of versions names.
pub const FORMAT: &str = "yardlog-batch/1";

/// The most records a batch carries.
pub const MOST_RECORDS: usize = 16;

/// The most bytes of entry files a batch carries, 1 MiB, unless one entry alone is larger. A
/// record is smaller than the entry that holds it, so the batch's body is no larger.
pub const MOST_BYTES: u64 = 1024 * 1024;

/// The record of `version` of `block`, in the stage `stage` of the Yard Log `yard_log`, as JSON
/// text. A stage's version names the stage as its block.
#[must_use]
pub fn record(yard_log: YardLogKey, stage: StageKey, block: &Block, version: &Version) -> String {
    let mut text = String::from("{\"record\":\"version\"");
    for (name, value) in [
        ("yardLog", yard_log.to_string()),
        ("stage", stage.to_string()),
        ("block", block.key.to_string()),
        ("kind", block.kind.as_str().to_owned()),
        ("id", block.id.as_str().to_owned()),
    ] {
        let _ = write!(text, ",{}:{}", string(name), string(&value));
    }
    text.push_str(",\"version\":");
    let Out::Mapping(fields) = write_version(version) else {
        unreachable!("a version is written as a mapping of fields")
    };
    let members = fields.iter().map(|(name, value)| (*name, value));
    separated(&mut text, ('{', '}'), members, |text, (name, value)| {
        member(text, name);
        match (&version.body, name) {
            (Body::Loft(loft), "report") => {
                text.push_str(&serde_json::Value::from(&loft.report).to_string());
            }
            _ => json(value, text),
        }
    });
    text.push('}');
    text
}

/// A batch of versions holding `records`, each the JSON text of a record, in order.
#[must_use]
pub fn batch<S: AsRef<str>>(records: &[S]) -> String {
    let mut text = format!("{{\"format\":{},\"records\":", string(FORMAT));
    separated(&mut text, ('[', ']'), records.iter(), |text, record| {
        text.push_str(record.as_ref());
    });
    text.push('}');
    text
}

/// `value` as JSON: a mapping as an object in the order written, a sequence as an array, text
/// and tokens as strings, an integer as a number, and a plain value as the JSON scalar it reads
/// as, or a string where it reads as none.
fn json(value: &Out, text: &mut String) {
    match value {
        Out::Mapping(fields) => object(fields.iter().map(|(name, value)| (*name, value)), text),
        Out::Map(entries) => object(
            entries.iter().map(|(name, value)| (name.as_str(), value)),
            text,
        ),
        Out::Sequence(items) => separated(text, ('[', ']'), items.iter(), |text, item| {
            json(item, text);
        }),
        Out::Token(value) | Out::Text(value) => text.push_str(&string(value)),
        Out::Integer(value) => {
            let _ = write!(text, "{value}");
        }
        Out::Plain(value) => match serde_json::from_str::<serde_json::Value>(value) {
            Ok(scalar @ (serde_json::Value::Bool(_) | serde_json::Value::Number(_))) => {
                text.push_str(&scalar.to_string());
            }
            _ => text.push_str(&string(value)),
        },
        Out::Null => text.push_str("null"),
    }
}

fn object<'a>(members: impl Iterator<Item = (&'a str, &'a Out)>, text: &mut String) {
    separated(text, ('{', '}'), members, |text, (name, value)| {
        member(text, name);
        json(value, text);
    });
}

/// `items` written by `each` between the brackets `open` and `close`, separated by commas.
fn separated<T>(
    text: &mut String,
    (open, close): (char, char),
    items: impl Iterator<Item = T>,
    mut each: impl FnMut(&mut String, T),
) {
    text.push(open);
    for (n, item) in items.enumerate() {
        if n > 0 {
            text.push(',');
        }
        each(text, item);
    }
    text.push(close);
}

/// The name of a member and its colon.
fn member(text: &mut String, name: &str) {
    text.push_str(&string(name));
    text.push(':');
}

/// `value` as a JSON string.
fn string(value: &str) -> String {
    serde_json::to_string(value).expect("a string always writes as JSON")
}
