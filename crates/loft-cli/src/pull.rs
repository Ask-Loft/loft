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
use loftspec::publish::{LOCK, Lock};
use loftspec_store::FileStore;
use loftspec_store::journal::{self, Changes, RootPath};
use serde_json::{Value, json};

use crate::finish::Finished;
use crate::json;
use crate::out::{errln, outln};
use crate::percent;
use crate::remote::head::{self, Head};
use crate::remote::{self, Client, RemoteError, files, history};
use crate::stop::Stop;

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-pull/1";

/// What is pulled.
pub enum Source {
    Head,
    Sequence(u64),
    Branch(String),
}

/// What the pull gives the tree: its files and lock, the conflicts the merge wrote, the fields
/// of its report, and whether it wrote anything.
struct Pulled {
    files: Files,
    lock: Option<Lock>,
    conflicts: Vec<Value>,
    fields: Value,
    written: bool,
}

impl Pulled {
    fn new(files: Files, lock: Option<Lock>, fields: Value) -> Self {
        Self {
            files,
            lock,
            conflicts: Vec::new(),
            fields,
            written: false,
        }
    }

    const fn outcome(&self) -> &'static str {
        if !self.written {
            "unchanged"
        } else if self.conflicts.is_empty() {
            "pulled"
        } else {
            "conflicted"
        }
    }
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
        Ok(pulled) => report(&pulled, as_json),
        Err(stop) => stop.exit(SCHEMA, as_json),
    }
}

fn report(pulled: &Pulled, as_json: bool) -> Finished {
    let outcome = pulled.outcome();
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
        let what = match pulled.fields["branch"].as_str() {
            Some(branch) => format!("the draft of the branch `{branch}`"),
            None => format!(
                "publish sequence {}, state root {}",
                pulled.fields["publishSequence"],
                pulled.fields["stateRoot"].as_str().unwrap_or("none")
            ),
        };
        if pulled.written {
            outln!("pulled {what}");
        } else {
            outln!("unchanged: the tree already holds {what}");
        }
    }
    if pulled.conflicts.is_empty() {
        Finished::success(outcome)
    } else {
        Finished::failure(outcome)
    }
}

/// Reads the tree, pulls what `source` names and writes it.
fn attempt(root: &Path, source: &Source) -> Result<Pulled, Stop> {
    let store = FileStore::open(root)?;
    let ours = store.read_files()?;
    let document = codec::read(&ours).map_err(Stop::from)?;
    if !document.connected {
        return Err(refused(
            "the document is not connected: only a connected document is pulled from its remote",
        ));
    }
    let lock = store.lock()?;
    if !matches!(source, Source::Head)
        && !lock
            .as_ref()
            .is_some_and(|lock| history::verifies(&ours, lock))
    {
        return Err(refused(
            "the tree is not the state loft.lock.json names: the pull would overwrite changes \
             in the working tree",
        ));
    }
    let key = document.id.key().to_string();
    let client =
        remote::connected(document.remote.as_deref(), &key).map_err(|error| unreached(&error))?;
    let head = head::fetch(&client, &key).map_err(|error| unreached(&error))?;
    let pulled = match source {
        Source::Head => {
            let Some(pulled) = from_head(&client, &ours, lock.as_ref(), &head, &key)? else {
                let at = Head::of_lock(lock.as_ref()).json();
                return Ok(Pulled::new(ours, lock, at));
            };
            pulled
        }
        Source::Sequence(sequence) => {
            let (files, state_lock) = state(&client, *sequence, || asked(*sequence))?;
            same_key(&files, &key)?;
            let fields = Head::of_lock(Some(&state_lock)).json();
            Pulled::new(files, Some(state_lock), fields)
        }
        Source::Branch(branch) => {
            let files = draft(&client, branch)?;
            same_key(&files, &key)?;
            Pulled::new(files, lock.clone(), json!({ "branch": branch }))
        }
    };
    let mut pulled = Pulled {
        files: connection_kept(pulled.files, &document)?,
        ..pulled
    };
    let changes = changes(&ours, &pulled, lock.as_ref());
    pulled.written = !changes.write.is_empty() || !changes.remove.is_empty();
    if pulled.written {
        journal::save(root, &changes)?;
    }
    Ok(pulled)
}

/// The head merged into `ours` three ways from the state the lock names; none where the head is
/// that state, or where neither the remote nor the tree holds a published state.
fn from_head(
    client: &Client,
    ours: &Files,
    lock: Option<&Lock>,
    head: &Value,
    key: &str,
) -> Result<Option<Pulled>, Stop> {
    let at = match Head::read(head) {
        Some(at) => at,
        None if head.is_null() => Head::none(),
        None => {
            return Err(unreached(&RemoteError::Unreachable {
                address: client.address().to_owned(),
                reason: "its head does not read".to_owned(),
            }));
        }
    };
    if at == Head::of_lock(lock) {
        return Ok(None);
    }
    let base = match lock {
        Some(lock) => {
            let missing = || {
                unreached(&RemoteError::StateNotHeld {
                    sequence: lock.publish_sequence.get(),
                    state_root: lock.state_root.to_string(),
                })
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
    if Some(head_lock.state_root.to_string()) != at.state_root {
        return Err(refused(format!(
            "the state the remote gave as its head is not its head, {at}"
        )));
    }
    same_key(&theirs, key)?;
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
        conflicts,
        ..Pulled::new(files, Some(head_lock), at.json())
    }))
}

/// The refusal of a pull of a publish sequence the remote does not hold.
fn asked(sequence: u64) -> Stop {
    refused(format!(
        "the remote holds no published state of publish sequence {sequence}"
    ))
}

/// The published state of `sequence`: its files under `spec/` and its lock, which they must
/// verify against; `missing` where the remote holds no such state.
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
    let files = wire_files(&state["files"])?;
    if lock.publish_sequence.get() != sequence || !history::verifies(&files, &lock) {
        return Err(refused(format!(
            "the state the remote gave as publish sequence {sequence} does not agree with its \
             lock"
        )));
    }
    Ok((files, lock))
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
    files::read(list, "gave").map_err(refused)
}

/// `files` with the manifest's `connected` and `remote` as `ours` has them, every file in the
/// bytes the formatter writes for it. The formatter marks departed the binding of a node the
/// tree no longer holds, as when ours deleted a node the pulled state still has.
fn connection_kept(mut files: Files, ours: &Document) -> Result<Files, Stop> {
    let formatted = format(&files).map_err(Stop::from)?;
    files.extend(formatted.changed);
    let mut document = codec::read(&files).map_err(Stop::from)?;
    document.connected = ours.connected;
    document.remote.clone_from(&ours.remote);
    let manifest = SpecPath::new(MANIFEST);
    let written = codec::write(&document)
        .remove(&manifest)
        .expect("the writer writes the manifest");
    files.insert(manifest, written);
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
