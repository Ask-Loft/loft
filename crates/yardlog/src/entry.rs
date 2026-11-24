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
pub enum Read {
    /// An entry this `loft` reads: its address and its record, as the text it holds.
    Entry { endpoint: String, record: String },
    /// An entry of a newer version of the format, for a newer `loft` to send.
    Newer,
    /// Not an entry: not JSON, another format, or an entry without its address or record.
    Foreign,
}

/// What `bytes` read as.
#[must_use]
pub fn read(bytes: &[u8]) -> Read {
    let Ok(fields) = serde_json::from_slice::<BTreeMap<String, Box<RawValue>>>(bytes) else {
        return Read::Foreign;
    };
    let field = |name: &str| fields.get(name).map(|raw| raw.get());
    let format = field("format").and_then(|raw| serde_json::from_str::<String>(raw).ok());
    let version = format
        .as_deref()
        .and_then(|format| format.strip_prefix(NAME))
        .and_then(|version| version.parse::<u32>().ok());
    match version {
        Some(VERSION) => {}
        Some(newer) if newer > VERSION => return Read::Newer,
        _ => return Read::Foreign,
    }
    let endpoint = field("to")
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|to| to.get("endpoint")?.as_str().map(str::to_owned));
    match (endpoint, field("record")) {
        (Some(endpoint), Some(record)) if record.starts_with('{') => Read::Entry {
            endpoint,
            record: record.to_owned(),
        },
        _ => Read::Foreign,
    }
}
