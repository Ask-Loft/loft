//! `loft pull`: brings a connected document's tree to its remote's head by a three-way merge,
//! or writes a published state or a branch's draft over a tree that is still the state its lock
//! names. Whatever it pulls, the manifest keeps the working tree's `connected` and `remote`,
//! every file is written in canonical bytes, and the tree, any removal and the lock go in one
//! journaled save.

use std::collections::BTreeSet;
use std::path::Path;

use loftspec::codec::{self, Files, MANIFEST, SpecPath};
use loftspec::document::Document;
use loftspec::fmt::format;
use loftspec::merge::merge_tree;
use loftspec::publish::{LOCK, Lock, ReadTree, verify};
use loftspec_store::FileStore;
use loftspec_store::journal::{self, Changes, RootPath};
use serde_json::{Value, json};

use crate::finish::Finished;
use crate::json;
use crate::out::{errln, outln};
use crate::percent;
use crate::remote::head::Head;
use crate::remote::{self, Client, RemoteError};
use crate::stop::Stop;

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-pull/1";

/// What is pulled.
pub enum Source {
    Head,
    Sequence(u64),
    Branch(String),
}

/// What the pull wrote: the files and lock it gives the tree, the conflicts the merge wrote, and
/// the fields of its report.
struct Pulled {
    files: Files,
    lock: Option<Lock>,
    conflicts: Vec<Value>,
    fields: Value,
}

/// A refusal of the pull.
fn refused(message: impl Into<String>) -> Stop {
    Stop::one("refused", message.into())
}

/// The remote's error as the refusal of the pull.
fn unreached(error: &RemoteError) -> Stop {
    refused(error.to_string())
}

pub fn run(root: &Path, source: &Source, as_json: bool) -> Finished {
    match attempt(root, source) {
        Ok(None) => {
            if as_json {
                json::print(&json::result(SCHEMA, "unchanged", json!({}), Vec::new()));
            } else {
                outln!("unchanged: the tree is at the remote's head");
            }
            Finished::success("unchanged")
        }
        Ok(Some(pulled)) => report(&pulled, as_json),
        Err(stop) => stop.exit(SCHEMA, as_json),
    }
}

fn report(pulled: &Pulled, as_json: bool) -> Finished {
    let outcome = if pulled.conflicts.is_empty() {
        "pulled"
    } else {
        "conflicted"
    };
    if as_json {
        let mut fields = pulled.fields.clone();
        fields["conflicts"] = json!(pulled.conflicts);
        json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
    } else {
        for conflict in &pulled.conflicts {
            errln!(
                "conflict {} in {}, field {}",
                conflict["id"].as_str().unwrap_or_default(),
                conflict["holder"].as_str().unwrap_or_default(),
                conflict["field"].as_str().unwrap_or_default()
            );
        }
        match pulled.fields["branch"].as_str() {
            Some(branch) => outln!("pulled the draft of the branch `{branch}`"),
            None => outln!(
                "pulled publish sequence {}, state root {}",
                pulled.fields["publishSequence"],
                pulled.fields["stateRoot"].as_str().unwrap_or("none")
            ),
        }
    }
    if pulled.conflicts.is_empty() {
        Finished::success(outcome)
    } else {
        Finished::failure(outcome)
    }
}

/// Reads the tree, pulls what `source` names and writes it; none where it writes nothing.
fn attempt(root: &Path, source: &Source) -> Result<Option<Pulled>, Stop> {
    let store = FileStore::open(root)?;
    let ours = store.read_files()?;
    let document = codec::read(&ours).map_err(Stop::from)?;
    if !document.connected {
        return Err(refused(
            "the document is not connected: only a connected document is pulled from its remote",
        ));
    }
    let lock = store.lock()?;
    if !matches!(source, Source::Head) && !at_lock(&ours, lock.as_ref()) {
        return Err(refused(
            "the tree is not the state loft.lock.json names: the pull would overwrite changes \
             in the working tree",
        ));
    }
    let key = document.id.key().to_string();
    let client =
        remote::connected(document.remote.as_deref(), &key).map_err(|error| unreached(&error))?;
    let head = head(&client, &key).map_err(|error| unreached(&error))?;
    let pulled = match source {
        Source::Head => match from_head(&client, &ours, lock.as_ref(), &head)? {
            Some(pulled) => pulled,
            None => return Ok(None),
        },
        Source::Sequence(sequence) => {
            let (files, state_lock) = state(&client, *sequence, || asked(*sequence))?;
            let fields = Head::of_lock(Some(&state_lock)).json();
            Pulled {
                files,
                lock: Some(state_lock),
                conflicts: Vec::new(),
                fields,
            }
        }
        Source::Branch(branch) => Pulled {
            files: draft(&client, branch)?,
            lock: lock.clone(),
            conflicts: Vec::new(),
            fields: json!({ "branch": branch }),
        },
    };
    let pulled = Pulled {
        files: connection_kept(pulled.files, &document)?,
        ..pulled
    };
    same_key(&pulled.files, &key)?;
    let changes = changes(&ours, &pulled, lock.as_ref());
    if changes.write.is_empty() && changes.remove.is_empty() {
        return Ok(None);
    }
    journal::save(root, &changes)?;
    Ok(Some(pulled))
}

/// Whether `files` are the state that `lock` names: a tree without a lock never is.
fn at_lock(files: &Files, lock: Option<&Lock>) -> bool {
    lock.is_some_and(|lock| ReadTree::read(files).is_ok_and(|tree| verify(&tree, lock).is_ok()))
}

/// The remote's head: `null`, or its publish sequence, state root and lock.
fn head(client: &Client, key: &str) -> Result<Value, RemoteError> {
    match client.get("head") {
        Ok(mut answer) => Ok(answer["head"].take()),
        Err(error) if error.refusal("not_found").is_some() => Err(RemoteError::NotHeld {
            key: key.to_owned(),
        }),
        Err(error) => Err(error),
    }
}

/// The head merged into `ours` three ways from the state the lock names; none where the head is
/// that state, or where neither the remote nor the tree holds a published state.
fn from_head(
    client: &Client,
    ours: &Files,
    lock: Option<&Lock>,
    head: &Value,
) -> Result<Option<Pulled>, Stop> {
    let at = Head::read(head).unwrap_or_else(Head::none);
    if at == Head::of_lock(lock) {
        return Ok(None);
    }
    let base = match lock {
        Some(lock) => {
            let missing = || {
                refused(format!(
                    "the remote holds no published state of publish sequence {}, state root {}",
                    lock.publish_sequence, lock.state_root
                ))
            };
            if head.is_null() {
                return Err(missing());
            }
            let (files, stored) = state(client, lock.publish_sequence.get(), missing)?;
            if stored.state_root != lock.state_root {
                return Err(missing());
            }
            files
        }
        None => Files::new(),
    };
    let (theirs, head_lock) = state(client, at.sequence, || asked(at.sequence))?;
    let merged = merge_tree(&base, ours, &theirs)
        .map_err(|error| refused(format!("the head does not merge: {error}")))?;
    let mut files = merged.files;
    // The head's bindings are the remote's, which alone gives labels.
    let labels = SpecPath::new(codec::LABELS);
    match theirs.get(&labels) {
        Some(bytes) => files.insert(labels, bytes.clone()),
        None => files.remove(&labels),
    };
    let conflicts = merged
        .document
        .conflicts
        .iter()
        .map(|conflict| {
            json!({
                "id": conflict.id().to_string(),
                "holder": conflict.owner.holder(),
                "field": conflict.field.to_string(),
            })
        })
        .collect();
    Ok(Some(Pulled {
        files,
        fields: at.json(),
        lock: Some(head_lock),
        conflicts,
    }))
}

/// The refusal of a pull of a publish sequence the remote does not hold.
fn asked(sequence: u64) -> Stop {
    refused(format!(
        "the remote holds no published state of publish sequence {sequence}"
    ))
}

/// The published state of `sequence`: its files under `spec/` and its lock; `missing` where the
/// remote holds no such state.
fn state(
    client: &Client,
    sequence: u64,
    missing: impl FnOnce() -> Stop,
) -> Result<(Files, Lock), Stop> {
    let state = client.get(&format!("states/{sequence}")).map_err(|error| {
        match error.refusal("not_found") {
            Some(_) => missing(),
            None => unreached(&error),
        }
    })?;
    let lock = Lock::read(state["lock"].to_string().as_bytes()).map_err(|error| {
        refused(format!(
            "the lock of publish sequence {sequence} does not read: {error}"
        ))
    })?;
    Ok((wire_files(&state["files"])?, lock))
}

/// The draft the remote holds under `branch`.
fn draft(client: &Client, branch: &str) -> Result<Files, Stop> {
    let draft = client
        .get(&format!("drafts/{}", percent::encode(branch)))
        .map_err(|error| match error.refusal("not_found") {
            Some(_) => refused(format!(
                "the remote holds no draft of the branch `{branch}`"
            )),
            None => unreached(&error),
        })?;
    wire_files(&draft["files"])
}

/// Files as the wire gives them, by their paths from the top of the tree, under `spec/`.
fn wire_files(list: &Value) -> Result<Files, Stop> {
    let mut files = Files::new();
    for file in list.as_array().into_iter().flatten() {
        let (Some(path), Some(text)) = (file["path"].as_str(), file["text"].as_str()) else {
            return Err(refused("the remote gave a file without its path or text"));
        };
        let under = path
            .strip_prefix("spec/")
            .filter(|under| {
                under
                    .split('/')
                    .all(|segment| !matches!(segment, "" | "." | ".."))
            })
            .ok_or_else(|| refused(format!("the remote gave a file outside spec/: {path}")))?;
        files.insert(SpecPath::new(under), text.as_bytes().to_vec());
    }
    Ok(files)
}

/// `files` with the manifest's `connected` and `remote` as `ours` has them, every file in the
/// bytes the formatter writes for it.
fn connection_kept(mut files: Files, ours: &Document) -> Result<Files, Stop> {
    let mut document = codec::read(&files).map_err(Stop::from)?;
    document.connected = ours.connected;
    document.remote.clone_from(&ours.remote);
    let manifest = SpecPath::new(MANIFEST);
    let written = codec::write(&document)
        .remove(&manifest)
        .expect("the writer writes the manifest");
    files.insert(manifest, written);
    let formatted = format(&files).map_err(Stop::from)?;
    files.extend(formatted.changed);
    Ok(files)
}

/// Refuses pulled files whose document key is not `key`.
fn same_key(files: &Files, key: &str) -> Result<(), Stop> {
    let pulled = codec::read(files).map_err(Stop::from)?.id.key().to_string();
    if pulled == key {
        return Ok(());
    }
    Err(refused(format!(
        "the pulled state's document key is {pulled}, but the manifest's is {key}"
    )))
}

/// The save that gives the tree `pulled`: each file that differs, each file of `ours` that it
/// does not hold removed, and its lock where that differs.
fn changes(ours: &Files, pulled: &Pulled, lock: Option<&Lock>) -> Changes {
    let write = pulled
        .files
        .iter()
        .filter(|(path, bytes)| ours.get(*path) != Some(*bytes))
        .map(|(path, bytes)| (RootPath::in_spec(path), bytes.clone()))
        .collect();
    let remove: BTreeSet<RootPath> = ours
        .keys()
        .filter(|path| !pulled.files.contains_key(*path))
        .map(RootPath::in_spec)
        .collect();
    let mut changes = Changes { write, remove };
    if let Some(pulled_lock) = &pulled.lock
        && lock != Some(pulled_lock)
    {
        changes
            .write
            .insert(RootPath::beside(LOCK), pulled_lock.to_bytes());
    }
    changes
}
