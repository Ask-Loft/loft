//! An outbox entry, `yardlog-outbox/1`: the copy of one version that waits on the machine until
//! Yard Server has taken it. It holds its format, the address of the Yard Server it is for as
//! `to.endpoint`, which never leaves the machine, and the version's record of a batch of
//! versions as `record`, which is read back as the text it is, so its fields keep their order.

use std::collections::BTreeMap;

use serde_json::value::RawValue;

/// The format an outbox entry names.
pub const FORMAT: &str = "yardlog-outbox/1";

/// The name of the format, before its version.
const NAME: &str = "yardlog-outbox/";

/// The version of the format this `loft` writes.
const VERSION: u32 = 1;

/// The text of an entry for the Yard Server at `endpoint`, holding `record`, the JSON text of a
/// record of a batch of versions.
#[must_use]
pub fn write(endpoint: &str, record: &str) -> String {
    let to = serde_json::Value::from(endpoint);
    format!("{{\"format\":\"{FORMAT}\",\"to\":{{\"endpoint\":{to}}},\"record\":{record}}}")
}

/// What the bytes of a file of the outbox read as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Read<'a> {
    /// An entry this `loft` reads: its address, its version's key and its record, as the text it
    /// holds.
    Entry {
        endpoint: String,
        key: String,
        record: &'a str,
    },
    /// An entry of a newer version of the format, for a newer `loft` to send.
    Newer,
    /// Not an entry: not JSON, another format, or an entry without its address, or without a record
    /// that holds its version's key.
    Foreign,
}

/// What `bytes` read as.
#[must_use]
pub fn read(bytes: &[u8]) -> Read<'_> {
    let Ok(fields) = serde_json::from_slice::<BTreeMap<String, &RawValue>>(bytes) else {
        return Read::Foreign;
    };
    let field = |name: &str| fields.get(name).map(|raw| raw.get());
    let format = field("format").and_then(|raw| serde_json::from_str::<String>(raw).ok());
    let version = format
        .as_deref()
        .and_then(|format| crate::format::versioned(format, NAME));
    match version {
        Some(VERSION) => {}
        Some(newer) if newer > VERSION => return Read::Newer,
        _ => return Read::Foreign,
    }
    let endpoint = field("to")
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|to| to.get("endpoint")?.as_str().map(str::to_owned));
    let (Some(endpoint), Some(record)) = (endpoint, field("record")) else {
        return Read::Foreign;
    };
    let Some(key) = version_key(record) else {
        return Read::Foreign;
    };
    Read::Entry {
        endpoint,
        key,
        record,
    }
}

/// The version's key that `record` holds, as each record of a batch must: Yard Server refuses a
/// whole batch where one record's key does not read.
fn version_key(record: &str) -> Option<String> {
    let key = field_of(field_of(record, "version")?, "key")?;
    serde_json::from_str(key).ok()
}

/// The text of the field `name` of the JSON object `text`, read without reading its values.
fn field_of<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let fields: BTreeMap<String, &'a RawValue> = serde_json::from_str(text).ok()?;
    fields.get(name).copied().map(RawValue::get)
}
