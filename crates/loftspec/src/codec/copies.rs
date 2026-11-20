//! The copies of other documents' Interfaces under `spec/documents/`: read from their bytes
//! apart from the tree's records, so that their keys, names and edges never reach the tree's own
//! resolution, and written from what another document's published state holds.

use std::collections::BTreeMap;

use super::error::{Problem, ReadError, SpecPath};
use super::files::{Content, Role, Tree};
use super::layout::{Sites, read_tree};
use super::record::{Mode, Rec, Val};
use super::resolve::{Resolver, resolve};
use super::schema::{Kind, field};
use super::{Files, write_content};
use crate::canonical::Digest;
use crate::copy::Copy;
use crate::document::Document;
use crate::hash::ContentHash;
use crate::identity::{Key, Label};
use crate::ids::{InterfaceId, NodeId};
use crate::node::Node;
use crate::slug::Slug;
use crate::yaml;

/// Why a copy could not be taken from another document's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyError {
    /// The state's files do not read.
    Unreadable(Vec<ReadError>),
    /// The state holds no Interface of that key.
    NotHeld(Key),
}

/// The copy of the Interface keyed `interface` that the files of another document's published
/// state hold, at `publish_sequence` and `state_root`, under `handle`.
///
/// # Errors
///
/// The files do not read, or hold no Interface of that key.
pub fn copy_of(
    handle: &Slug,
    owner: &Files,
    interface: Key,
    publish_sequence: u64,
    state_root: Digest,
) -> Result<Copy, CopyError> {
    let tree = read_tree(owner).map_err(CopyError::Unreadable)?;
    let document = resolve(&tree).map_err(CopyError::Unreadable)?;
    let id = InterfaceId::new(interface);
    let held = (document.interfaces.iter())
        .find(|held| held.id == Some(id))
        .ok_or(CopyError::NotHeld(interface))?;
    let source = Role::Interface {
        slug: held.slug.as_str().to_owned(),
    };
    let record = tree
        .record(&source.path())
        .expect("a read Interface has its file");
    let mut labels = Vec::new();
    Node::Interface(held).walk(&mut |node| {
        if let Some(id) = node.id()
            && let Some(label) = document.bindings.label_of_key(id.key())
        {
            labels.push(label_record(id.key(), label));
        }
    });
    let mut copy = Rec::new(Kind::Copy);
    copy.set_text(field::PUBLISH_SEQUENCE, publish_sequence.to_string());
    copy.set_text(field::STATE_ROOT, state_root.to_string());
    copy.set_text(field::INTERFACE_HASH, held.content_hash().to_string());
    copy.set(
        field::LABELS,
        Val::List(labels.into_iter().map(Val::Record).collect()),
    );
    copy.set(field::INTERFACE, Val::Record(record.clone()));
    let role = Role::Copy {
        handle: handle.as_str().to_owned(),
        slug: held.slug.as_str().to_owned(),
    };
    let path = role.path();
    let bytes = write_content(&path, role.clone(), Content::Record(copy));
    read(&path, &role, &bytes).map_err(|error| CopyError::Unreadable(vec![error]))
}

fn label_record(key: Key, label: Label) -> Rec {
    let mut record = Rec::new(Kind::CopyLabel);
    record.set_text(field::KEY, key.to_string());
    record.set_text(field::LABEL, label.to_string());
    record
}

/// The copy a file of the role `Role::Copy` holds.
///
/// # Errors
///
/// The file does not read as a copy, its Interface's slug is not the file's name, or its
/// Interface does not hash to its `interfaceHash`.
pub(super) fn read(path: &SpecPath, role: &Role, bytes: &[u8]) -> Result<Copy, ReadError> {
    let Role::Copy { handle, slug } = role else {
        unreachable!("a copy's role");
    };
    let node = yaml::read(bytes)
        .map_err(|error| ReadError::yaml(path, error))?
        .ok_or_else(|| ReadError::at(path, None, Problem::Expected("a copy; the file is empty")))?;
    let record = super::record::check(&node, Kind::Copy, Mode::Strict)
        .map_err(|found| found.into_error(path))?;
    let held = record.record(field::INTERFACE).expect("a checked copy");
    let interface = Resolver::foreign(path).interface(&without_mentions(held))?;
    let model = |problem: String| ReadError::at(path, Some(held.position), Problem::Model(problem));
    if interface.slug.as_str() != slug {
        let named = interface.slug.as_str();
        return Err(model(format!(
            "the copy's Interface has the slug {named}, not {slug}"
        )));
    }
    let digest = |name| {
        text(&record, name)
            .parse::<Digest>()
            .expect("a checked digest")
    };
    let interface_hash = digest(field::INTERFACE_HASH);
    if interface.content_hash() != interface_hash {
        return Err(ReadError::at(path, None, Problem::Model(
            "the copy's Interface does not hash to its `interfaceHash`: it was edited, and only \
             `loft interface` writes a copy"
                .to_owned(),
        )));
    }
    let labels = (record.records(field::LABELS))
        .map(|entry| {
            let key = Key::parse(&text(entry, field::KEY)).expect("a checked key");
            (
                key,
                Label::parse(&text(entry, field::LABEL)).expect("a checked label"),
            )
        })
        .collect();
    Ok(Copy {
        handle: Slug::parse(handle).map_err(|error| {
            ReadError::at(
                path,
                None,
                Problem::Model(format!(
                    "the copy's directory {handle} is no handle: {error}"
                )),
            )
        })?,
        publish_sequence: text(&record, field::PUBLISH_SEQUENCE)
            .parse()
            .expect("a checked integer"),
        state_root: digest(field::STATE_ROOT),
        interface_hash,
        labels,
        interface,
        bytes: bytes.to_vec(),
    })
}

fn text(record: &Rec, name: &str) -> String {
    record.text(name).unwrap_or_default().to_owned()
}

/// The record with every `mentions` beneath it taken out: a copy's edges name the other
/// document's own elements, and no node's content holds an edge.
fn without_mentions(record: &Rec) -> Rec {
    let mut record = record.clone();
    strip(&mut record);
    record
}

fn strip(record: &mut Rec) {
    record.fields.retain(|(name, _)| *name != field::MENTIONS);
    record.children_mut().for_each(strip);
}

/// The copies of the tree, each read from its file.
pub(super) fn copies(tree: &Tree) -> (Vec<Copy>, Vec<ReadError>) {
    let mut copies = Vec::new();
    let mut errors = Vec::new();
    for (path, (role, content)) in &tree.files {
        let (Role::Copy { .. }, Content::Bytes(bytes)) = (role, content) else {
            continue;
        };
        match read(path, role, bytes) {
            Ok(copy) => copies.push(copy),
            Err(error) => errors.push(error),
        }
    }
    (copies, errors)
}

/// The node of each copy, by the copy's handle and the node's key.
pub(super) fn foreign(copies: &[Copy]) -> BTreeMap<(String, Key), NodeId> {
    let mut nodes = BTreeMap::new();
    for copy in copies {
        for id in copy.ids() {
            nodes.insert((copy.handle.as_str().to_owned(), id.key()), id);
        }
    }
    nodes
}

/// Each copy under a handle the manifest's `documents` does not list, and each key that a copy
/// carries with an element of the document or with another copy.
pub(super) fn crossing(tree: &Tree, document: &Document) -> Vec<ReadError> {
    let mut errors = Vec::new();
    for copy in &document.copies {
        let listed = document.connected
            && (document.documents.iter()).any(|other| other.handle == copy.handle);
        if !listed {
            let directory = SpecPath::new(&format!("documents/{}", copy.handle));
            let problem = format!(
                "the copies under {} belong to no other document: the manifest's `documents` \
                 does not list the handle {}, and only a connected document links to other \
                 documents",
                directory, copy.handle
            );
            errors.push(ReadError::at(&directory, None, Problem::Model(problem)));
        }
    }
    if document.copies.is_empty() {
        return errors;
    }
    let sites = Sites::of(tree);
    let mut seen: BTreeMap<Key, SpecPath> = BTreeMap::new();
    for copy in &document.copies {
        let path = copy.path();
        for key in copy.keys() {
            let first = match (sites.get(key), seen.get(&key)) {
                (Some(site), _) => site.clone(),
                (None, Some(first)) => super::Site {
                    path: first.clone(),
                    location: None,
                },
                (None, None) => {
                    seen.insert(key, path.clone());
                    continue;
                }
            };
            let other = super::Site {
                path: path.clone(),
                location: None,
            };
            errors.push(ReadError {
                sites: vec![first, other],
                problem: Problem::DuplicateKey(key),
            });
        }
    }
    errors
}
