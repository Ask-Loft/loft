//! The files each session writes alone in Yard Log's folder: its token totals under `tokens/` and
//! its activity under `activity/`. Only that session writes one, so two copies differ only where
//! one has seen more of its events, and the merge driver takes one whole rather than joining
//! them; all it reads of a copy is when its last event ended.

use super::reader::{Fields, time};
use super::{FileError, parse};
use crate::act::Time;

/// The folder of a session's token totals.
pub const TOKENS: &str = "tokens";
/// The folder of a session's activity.
pub const ACTIVITY: &str = "activity";
/// The field that holds when a session's file's last event ended.
pub const LAST_ENDED: &str = "lastEnded";

/// When the last event that a session's file holds ended, where it holds one.
///
/// # Errors
///
/// The file is not a mapping, or its `lastEnded` is not a time.
pub fn last_ended(name: &str, bytes: &[u8]) -> Result<Option<Time>, FileError> {
    let in_file = |error| FileError {
        file: name.to_owned(),
        error,
    };
    let node = parse(name, bytes)?;
    let mut fields = Fields::of(&node).map_err(in_file)?;
    fields
        .get(LAST_ENDED)
        .map(time)
        .transpose()
        .map_err(in_file)
}
