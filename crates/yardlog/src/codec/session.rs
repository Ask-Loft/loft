//! The files each session writes alone in Yard Log's folder: its token totals under `tokens/`,
//! its activity under `activity/` and what its agents consulted under `consultation/`. Only that
//! session writes one, so two copies differ only where one has seen more of its events, and the
//! merge driver takes one whole rather than joining them; all it reads of a copy is when its last
//! event ended.

use super::reader::{Fields, time};
use super::{FileError, parse};
use crate::act::Time;
use crate::consult::Consultation;
use crate::totals::TotalsFormat;

/// The folder of a session's token totals.
pub const TOKENS: &str = "tokens";
/// The folder of a session's activity.
pub const ACTIVITY: &str = "activity";
/// The folder of what a session's agents consulted.
pub const CONSULTATION: &str = "consultation";
/// Every folder of the files a session writes alone.
pub const FOLDERS: [&str; 3] = [TOKENS, ACTIVITY, CONSULTATION];
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

/// Whether this `loft` writes over a session's file of `folder` that holds `bytes`: not where it
/// is a totals file of a newer totals format or of one it does not know, nor a consultation file
/// of a format other than its own, nor one whose root is no mapping, which names no format. A file
/// that does not read is written over, since only its own session's events change it and the next
/// of them writes it whole.
#[must_use]
pub fn writes_over(folder: &str, bytes: &[u8]) -> bool {
    if folder != TOKENS && folder != CONSULTATION {
        return true;
    }
    let Ok(node) = parse(folder, bytes) else {
        return true;
    };
    let Ok(mut fields) = Fields::of(&node) else {
        // A totals file that names no format is of the first totals format.
        return folder == TOKENS;
    };
    let Ok(format) = fields.optional_text("format") else {
        return false;
    };
    if folder == TOKENS {
        TotalsFormat::named(format.as_deref()).is_some_and(TotalsFormat::is_known)
    } else {
        Consultation::is_format(format.as_deref())
    }
}
