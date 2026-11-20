//! `loft interface add <handle>:<interface>`: a copy of another document's Interface, as that
//! document published it at its remote's head or at the publish sequence `--at` names, listing
//! the other document in the manifest the first time.

use std::num::NonZeroU64;
use std::path::Path;

use loftspec::codec::{self, CopyError, Files, MANIFEST, SpecPath};
use loftspec::document::{Document, OtherDocument};
use loftspec::identity::Key;
use loftspec::ids::{DocumentId, ElementId, NodeId};
use loftspec::slug::Slug;
use serde_json::json;

use super::reach::{head_sequence, not_held, state};
use super::{Interface, Named, Report, connected, document, named, refused, save};
use crate::remote;
use crate::stop::Stop;

pub(super) fn run(
    root: &Path,
    text: &str,
    at: Option<NonZeroU64>,
    remote: Option<&str>,
    browser: bool,
) -> Result<Report, Stop> {
    let Named { handle, interface } = named(text)?;
    let (store, mut document) = document(root)?;
    connected(&document)?;
    let listed = (document.documents.iter()).find(|entry| entry.handle == handle);
    let address = address(&handle, listed, remote)?;
    let client = remote::other(&address, browser)?;
    let sequence = match at {
        Some(at) => at.get(),
        None => head_sequence(&client, &handle)?,
    };
    let (files, lock) = state(&client, sequence)?.ok_or_else(|| not_held(&address, sequence))?;
    let other = lock.document_key;
    served(&document, listed, other, &address, &handle)?;
    let missing = || {
        refused(format!(
            "{handle}: the published state of publish sequence {sequence} holds no Interface \
             {interface}"
        ))
    };
    let key = interface_key(&files, interface).ok_or_else(missing)?;
    if (document.copies.iter()).any(|copy| copy.handle == handle && copy.keys()[0] == key) {
        return Err(refused(format!(
            "the document already holds a copy of {handle}:{interface}: `loft interface update` \
             moves its pin"
        )));
    }
    let copy = codec::copy_of(&handle, &files, key, sequence, lock.state_root).map_err(
        |error| match error {
            CopyError::NotHeld(_) => missing(),
            CopyError::Unreadable(errors) => refused(format!(
                "{handle}: the published state of publish sequence {sequence} does not read: {}",
                errors.first().map(ToString::to_string).unwrap_or_default()
            )),
        },
    )?;
    let first = listed.is_none();
    if first {
        document.documents.push(OtherDocument {
            handle: handle.clone(),
            key: other,
            remote: address,
        });
    }
    let name = copy.labels.get(&key).map_or_else(
        || format!("{handle}:{key}"),
        |label| format!("{handle}:{label}"),
    );
    let path = copy.path();
    document.copies.push(copy);
    save(&store, &document, &[SpecPath::new(MANIFEST), path.clone()])?;
    Ok(added(&name, key, sequence, &path, first))
}

/// The address of the other document's remote: its entry's, or `remote` for a handle that
/// `documents` does not list.
fn address(
    handle: &Slug,
    listed: Option<&OtherDocument>,
    remote: Option<&str>,
) -> Result<String, Stop> {
    match (listed, remote) {
        (Some(entry), Some(given)) if entry.remote != given => Err(refused(format!(
            "`documents` lists {handle} at {}, not {given}",
            entry.remote
        ))),
        (Some(entry), _) => Ok(entry.remote.clone()),
        (None, Some(given)) => Ok(given.to_owned()),
        (None, None) => Err(refused(format!(
            "`documents` does not list {handle}: `--remote <address>` lists the other document"
        ))),
    }
}

/// Refuses a remote at `address` that serves `document` itself, or a document other than the
/// one `listed` names.
fn served(
    document: &Document,
    listed: Option<&OtherDocument>,
    other: DocumentId,
    address: &str,
    handle: &Slug,
) -> Result<(), Stop> {
    if other == document.id {
        return Err(refused(format!(
            "the remote at {address} serves the document {} itself, which cannot link to its \
             own Interfaces",
            other.key()
        )));
    }
    match listed.filter(|entry| entry.key != other) {
        Some(entry) => Err(refused(format!(
            "the remote at {address} serves the document {}, not {} that `documents` lists \
             under {handle}",
            other.key(),
            entry.key.key()
        ))),
        None => Ok(()),
    }
}

/// The report of a copy of `name` added at `sequence` to `path`, `first` where it listed the
/// other document.
fn added(name: &str, key: Key, sequence: u64, path: &SpecPath, first: bool) -> Report {
    Report {
        outcome: "added",
        fields: json!({
            "interface": name,
            "key": key.to_string(),
            "publishSequence": sequence,
            "path": format!("spec/{}", path.as_str()),
            "listed": first,
        }),
        lines: vec![
            format!("added a copy of {name} at publish sequence {sequence}"),
            format!("Commit spec/: it now holds a copy of {name}."),
        ],
        failed: false,
    }
}

/// The key of the Interface `interface` names in the document whose published files are
/// `files`: the Interface its label is bound to, or the key itself.
fn interface_key(files: &Files, interface: Interface) -> Option<Key> {
    match interface {
        Interface::Key(key) => Some(key),
        Interface::Label(label) => {
            let other: Document = codec::read(files).ok()?;
            match other.bindings.resolve(label)? {
                ElementId::Node(NodeId::Interface(id)) => Some(id.key()),
                _ => None,
            }
        }
    }
}
