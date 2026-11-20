//! A published state as the remote serves it under `states/<sequence>`: its files under `spec/`
//! and its lock, which they must verify against. Pull, publish and `loft interface` take states
//! through it, from the document's own remote and from another document's.

use loftspec::codec::Files;
use loftspec::publish::Lock;

use super::{Client, RemoteError, files, history};

/// Why a published state could not be taken.
#[derive(Debug)]
pub enum StateError {
    /// The remote holds no published state of the sequence.
    Missing,
    /// The remote could not be reached, or refused the call.
    Remote(RemoteError),
    /// The remote gave a state that does not read, or that disagrees with its lock.
    Disagrees(String),
}

/// The published state of `sequence` at the remote `client` reaches: its files under `spec/` and
/// its lock, which they verify against.
///
/// # Errors
///
/// The remote holds no such state, cannot be reached or refuses the call, or gives a state whose
/// lock does not read or does not agree with its files and its sequence.
pub fn state(client: &Client, sequence: u64) -> Result<(Files, Lock), StateError> {
    let state = client.get(&format!("states/{sequence}")).map_err(|error| {
        match error.refusal("not_found") {
            Some(_) => StateError::Missing,
            None => StateError::Remote(error),
        }
    })?;
    let lock = Lock::read(state["lock"].to_string().as_bytes()).map_err(|error| {
        StateError::Disagrees(format!(
            "the lock of publish sequence {sequence} does not read: {error}"
        ))
    })?;
    let files = files::read(&state["files"], "gave").map_err(StateError::Disagrees)?;
    if lock.publish_sequence.get() != sequence || !history::verifies(&files, &lock) {
        return Err(StateError::Disagrees(format!(
            "the state the remote gave as publish sequence {sequence} does not agree with its \
             lock"
        )));
    }
    Ok((files, lock))
}
