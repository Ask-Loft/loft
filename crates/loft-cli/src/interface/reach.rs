//! Reaching another document's remote: the head's publish sequence and a published state, each
//! refusal said in the command's words.

use loftspec::codec::Files;
use loftspec::publish::Lock;
use loftspec::slug::Slug;

use super::refused;
use crate::remote::Client;
use crate::remote::head::{self, Head};
use crate::remote::states::{self, StateError};
use crate::stop::Stop;

/// The publish sequence of the head of the remote `client` reaches.
pub(super) fn head_sequence(client: &Client, handle: &Slug) -> Result<u64, Stop> {
    let head = head::fetch(client, handle.as_str())?;
    match Head::read(&head) {
        Some(head) if head.sequence > 0 => Ok(head.sequence),
        _ => Err(refused(format!(
            "{handle}: the remote at {} holds no published state",
            client.address()
        ))),
    }
}

/// The published state of `sequence` at the remote `client` reaches.
pub(super) fn state(client: &Client, sequence: u64) -> Result<(Files, Lock), Stop> {
    states::state(client, sequence).map_err(|error| match error {
        StateError::Missing => refused(format!(
            "the remote at {} holds no published state of publish sequence {sequence}",
            client.address()
        )),
        StateError::Remote(error) => error.into(),
        StateError::Disagrees(reason) => refused(reason),
    })
}
