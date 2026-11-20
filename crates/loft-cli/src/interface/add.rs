//! `loft interface add <handle>:<interface>`: a copy of another document's Interface, as that
//! document published it at its remote's head or at the publish sequence `--at` names, listing
//! the other document in the manifest the first time.

use std::num::NonZeroU64;
use std::path::Path;

use loftspec::codec::{self, CopyError, Files, MANIFEST, SpecPath};
use loftspec::copy::Copy;
use loftspec::document::{Document, OtherDocument};
use loftspec::identity::Key;
use loftspec::ids::{ElementId, NodeId};
use loftspec::publish::Lock;
use loftspec::slug::Slug;
use serde_json::json;

use super::reach::{another, head_sequence, not_held, state, unreadable};
use super::{Interface, Named, Report, clashing, connected, document, named, refused, save};
use crate::remote::{self, SignIn};
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
    let client = remote::other(&address, SignIn::First { browser })?;
    let sequence = match at {
        Some(at) => at.get(),
        None => head_sequence(&client, &handle)?,
    };
    let (files, lock) = state(&client, sequence)?.ok_or_else(|| not_held(&address, sequence))?;
    let other = lock.document_key;
    served(&document, listed, &lock, &address)?;
    let missing = || {
        refused(format!(
            "{handle}: the published state of publish sequence {sequence} holds no Interface \
             {interface}"
        ))
    };
    let key = interface_key(&files, interface, &handle, sequence)?.ok_or_else(missing)?;
    if (document.copies.iter()).any(|copy| copy.handle == handle && copy.key() == key) {
        return Err(refused(format!(
            "the document already holds a copy of {handle}:{interface}: `loft interface update` \
             moves its pin"
        )));
    }
    let copy = codec::copy_of(&handle, &files, key, sequence, lock.state_root).map_err(
        |error| match error {
            CopyError::NotHeld(_) => missing(),
            CopyError::Unreadable(errors) => unreadable(&handle, sequence, &errors),
        },
    )?;
    let mut after: Vec<&Copy> = document.copies.iter().collect();
    after.push(&copy);
    clashing(&document, &after)?;
    let first = listed.is_none();
    if first {
        document.documents.push(OtherDocument {
            handle: handle.clone(),
            key: other,
            remote: address,
        });
    }
    let name = copy.name_of(key);
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
    lock: &Lock,
    address: &str,
) -> Result<(), Stop> {
    let other = lock.document_key;
    if other == document.id {
        return Err(refused(format!(
            "the remote at {address} serves the document {} itself, which cannot link to its \
             own Interfaces",
            other.key()
        )));
    }
    if let Some(entry) = listed.filter(|entry| entry.key != other) {
        return Err(another(entry, lock));
    }
    match (
        listed,
        document.documents.iter().find(|entry| entry.key == other),
    ) {
        (None, Some(entry)) => Err(refused(format!(
            "the remote at {address} serves the document {}, which `documents` lists under {} \
             already: name the Interface as {}:<interface>",
            other.key(),
            entry.handle,
            entry.handle
        ))),
        _ => Ok(()),
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

/// The key of the Interface `interface` names in the document whose published files at
/// `sequence` are `files`: the Interface its label is bound to, or the key itself; none where no
/// Interface carries the label.
fn interface_key(
    files: &Files,
    interface: Interface,
    handle: &Slug,
    sequence: u64,
) -> Result<Option<Key>, Stop> {
    let label = match interface {
        Interface::Key(key) => return Ok(Some(key)),
        Interface::Label(label) => label,
    };
    let other = codec::read(files).map_err(|errors| unreadable(handle, sequence, &errors))?;
    Ok(match other.bindings.resolve(label) {
        Some(ElementId::Node(NodeId::Interface(id))) => Some(id.key()),
        _ => None,
    })
}
