//! The event step of `loft yard upgrade`: in the event log of each Codex session of an open Yard
//! Log, each zero cache-write count of a request or compaction that no verified API-key
//! account reported moves from its tokens to `usageSource.reported`, as the adapter now gives
//! it, so that replaying the log gives the totals the upgrade wrote. Every other field of the
//! line, its event identity and the digest of its original payload among them, is kept, so a
//! retry of the original event is still recognized; every other line keeps its bytes.

use serde_json::{Value, json};
use yardlog_store::home::Home;

use crate::yard::failure::Failure;

/// Whether session `key`'s event log holds a count to move.
pub fn pending(home: &Home, key: &str) -> Result<bool, Failure> {
    let Some(bytes) = home.read_events(key)? else {
        return Ok(false);
    };
    Ok(bytes
        .split_inclusive(|byte| *byte == b'\n')
        .any(|line| moved(line).is_some()))
}

/// Rewrites session `key`'s event log, where it holds a count to move.
pub fn settle(home: &Home, key: &str) -> Result<(), Failure> {
    let Some(bytes) = home.read_events(key)? else {
        return Ok(());
    };
    let mut changed = false;
    let mut settled = Vec::with_capacity(bytes.len());
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        match moved(line) {
            Some(line) => {
                settled.extend(line);
                changed = true;
            }
            None => settled.extend_from_slice(line),
        }
    }
    if changed {
        home.replace_events(key, &settled)?;
    }
    Ok(())
}

/// The line `line` with its zero cache write moved, or none where it holds none to move.
fn moved(line: &[u8]) -> Option<Vec<u8>> {
    let text = line.strip_suffix(b"\n").unwrap_or(line);
    let mut event: Value = serde_json::from_slice(text).ok()?;
    let counted = matches!(event["kind"].as_str(), Some("request" | "compaction"));
    if !counted || event["tokens"]["cacheWrite"] != 0 || event["usageSource"]["account"] == "apiKey"
    {
        return None;
    }
    event["tokens"].as_object_mut()?.remove("cacheWrite");
    let fields = event.as_object_mut()?;
    let source = fields.entry("usageSource").or_insert_with(|| json!({}));
    source["reported"] = json!({ "cacheWrite": 0 });
    let mut written = serde_json::to_vec(&event).ok()?;
    if line.ends_with(b"\n") {
        written.push(b'\n');
    }
    Some(written)
}
