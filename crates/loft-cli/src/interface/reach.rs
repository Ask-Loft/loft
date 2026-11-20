//! Reaching other documents' remotes: each once per command, for its client, the publish
//! sequence of its head and each published state taken from it, each refusal said in the
//! command's words.

use std::collections::BTreeMap;

use loftspec::codec::{Files, ReadError};
use loftspec::document::OtherDocument;
use loftspec::publish::Lock;
use loftspec::slug::Slug;

use super::refused;
use crate::remote::head::{self, Head};
use crate::remote::states::{self, StateError};
use crate::remote::{self, Client};
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

/// The published state of `sequence` at the remote `client` reaches, or none where it holds
/// none.
pub(super) fn state(client: &Client, sequence: u64) -> Result<Option<State>, Stop> {
    match states::state(client, sequence) {
        Ok(taken) => Ok(Some(taken)),
        Err(StateError::Missing) => Ok(None),
        Err(StateError::Remote(error)) => Err(error.into()),
        Err(StateError::Disagrees(reason)) => Err(refused(reason)),
    }
}

/// The refusal of a state the remote at `address` does not hold.
pub(super) fn not_held(address: &str, sequence: u64) -> Stop {
    refused(format!(
        "the remote at {address} holds no published state of publish sequence {sequence}"
    ))
}

/// The refusal of a published state of `handle` at `sequence` that does not read: `errors` are
/// the reader's.
pub(super) fn unreadable(handle: &Slug, sequence: u64, errors: &[ReadError]) -> Stop {
    refused(format!(
        "{handle}: the published state of publish sequence {sequence} does not read: {}",
        errors.first().map(ToString::to_string).unwrap_or_default()
    ))
}

/// The refusal of a remote that serves `served` in place of the document `entry` lists.
pub(super) fn another(entry: &OtherDocument, served: &Lock) -> Stop {
    refused(format!(
        "the remote at {} serves the document {}, not {} that `documents` lists under {}",
        entry.remote,
        served.document_key.key(),
        entry.key.key(),
        entry.handle
    ))
}

/// A published state: its files under `spec/` and its lock.
type State = (Files, Lock);

/// The remotes of other documents a command reaches, each once: its client, the publish
/// sequence of its head, and each state taken from it, none where the remote holds none.
pub(super) struct Remotes {
    browser: bool,
    clients: BTreeMap<Slug, Client>,
    heads: BTreeMap<Slug, u64>,
    states: BTreeMap<(Slug, u64), Option<State>>,
}

impl Remotes {
    /// The remotes, reached with a sign-in that opens the person's browser where `browser`.
    pub fn new(browser: bool) -> Self {
        Self {
            browser,
            clients: BTreeMap::new(),
            heads: BTreeMap::new(),
            states: BTreeMap::new(),
        }
    }

    fn client(&mut self, entry: &OtherDocument) -> Result<&Client, Stop> {
        if !self.clients.contains_key(&entry.handle) {
            let client = remote::other(&entry.remote, self.browser)?;
            self.clients.insert(entry.handle.clone(), client);
        }
        Ok(&self.clients[&entry.handle])
    }

    /// The publish sequence of the head of the remote of `entry`.
    pub fn head(&mut self, entry: &OtherDocument) -> Result<u64, Stop> {
        if let Some(head) = self.heads.get(&entry.handle) {
            return Ok(*head);
        }
        let head = head_sequence(self.client(entry)?, &entry.handle)?;
        self.heads.insert(entry.handle.clone(), head);
        Ok(head)
    }

    /// The published state of `sequence` at the remote of `entry`, or none where the remote
    /// holds none.
    ///
    /// # Errors
    ///
    /// The remote cannot be reached or refuses, gives a state that disagrees with its lock, or
    /// serves a document other than the one `entry` lists.
    pub fn state(&mut self, entry: &OtherDocument, sequence: u64) -> Result<Option<&State>, Stop> {
        let key = (entry.handle.clone(), sequence);
        if !self.states.contains_key(&key) {
            let taken = state(self.client(entry)?, sequence)?;
            if let Some((_, lock)) = &taken
                && lock.document_key != entry.key
            {
                return Err(another(entry, lock));
            }
            self.states.insert(key.clone(), taken);
        }
        Ok(self.states[&key].as_ref())
    }
}
