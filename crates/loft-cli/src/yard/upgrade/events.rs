//! The event step of `loft yard upgrade`: in every session's event log in `~/.yard/events/`,
//! whichever tree's Yard Log it counts toward and whatever its totals file's format, each request
//! or compaction a Codex session posted has its zero cache-write count moved by the Codex
//! adapter's rule, from its tokens to `usageSource.reported`, as the adapter now gives it, so that
//! replaying the log gives the totals the upgrade wrote. Every other field of the line, its event
//! identity and the digest of its original payload among them, is kept, so a retry of the
//! original event is still recognized; every other line keeps its bytes.

use serde_json::Value;
use yardlog_store::YardStoreError;
use yardlog_store::home::Home;
use yardlog_store::lock::Locked;

use crate::yard::codex::classify_event;

/// A zero cache-write count, as the page server writes it in a line of an event log.
const ZERO_WRITE: &[u8] = b"\"cacheWrite\":0";

/// Whether session `key`'s event log holds a count to move.
pub fn pending(home: &Home, key: &str) -> Result<bool, YardStoreError> {
    let Some(bytes) = home.read_events(key)? else {
        return Ok(false);
    };
    Ok(bytes
        .split_inclusive(|byte| *byte == b'\n')
        .any(|line| moved(line).is_some()))
}

/// Rewrites session `key`'s event log, where it holds a count to move.
/// `server` is the page server's lock, held meanwhile.
pub fn settle(home: &Home, server: &Locked, key: &str) -> Result<(), YardStoreError> {
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
        home.replace_events(server, key, &settled)?;
    }
    Ok(())
}

/// The line `line` with its zero cache write moved by the Codex adapter's rule, or none where it
/// holds none to move. Only a line that names a zero cache write, as the page server writes it,
/// is parsed.
fn moved(line: &[u8]) -> Option<Vec<u8>> {
    if !line
        .windows(ZERO_WRITE.len())
        .any(|part| part == ZERO_WRITE)
    {
        return None;
    }
    let text = line.strip_suffix(b"\n").unwrap_or(line);
    let mut event: Value = serde_json::from_slice(text).ok()?;
    if event["host"] != "codex" || !classify_event(&mut event) {
        return None;
    }
    let mut written = serde_json::to_vec(&event).ok()?;
    if line.ends_with(b"\n") {
        written.push(b'\n');
    }
    Some(written)
}
