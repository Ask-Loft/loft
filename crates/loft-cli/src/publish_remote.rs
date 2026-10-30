//! The publish of a connected document, through its remote: `loft publish` asks for the remote's
//! head, takes the base the lock names from the head, the history or the remote, sends the
//! change from it as a delta under a retry key, and applies the labels, files and lock the remote
//! returns once the roots it recomputes agree with that lock. The publisher never runs here as
//! the document's remote.

use std::collections::BTreeMap;
use std::path::Path;

use loftspec::canonical::canonical_bytes;
use loftspec::codec::{Files, SpecPath};
use loftspec::document::Document;
use loftspec::publish::{Lock, ReadTree, Unverified, verify};
use loftspec_store::FileStore;
use loftspec_store::repository::GitCommand;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::remote::{self, Client, RemoteError};

/// What a connected document is, as every refusal to publish one names it.
pub const CONNECTED: &str = "the document is connected: a connected document is published only \
                             through its remote, and its labels come from its remote";

/// The bindings, which the remote writes and no delta carries.
const LABELS: &str = "labels.yaml";

/// What a connected publish came to.
pub enum Outcome {
    /// The remote published the change, or replayed a publish it had made: its lock and the
    /// labels it gave, each with its key.
    Published {
        lock: Lock,
        labels: Vec<(String, String)>,
    },
    /// There was nothing to publish: the head's publish sequence and state root.
    Unchanged { sequence: u64, state_root: String },
}

/// A head or a base by its publish sequence and state root, as the wire names them.
fn mark(sequence: &Value, state_root: &Value) -> Value {
    json!({ "publishSequence": sequence, "stateRoot": state_root })
}

/// Publishes the tree under `root`, whose files are `files`, whose document is `document` and
/// whose state root is `state_root`, through its remote.
///
/// # Errors
///
/// The remote cannot be reached or refuses, the base cannot be found, or the returned lock does
/// not agree with the tree it returns.
pub fn publish(
    root: &Path,
    store: &FileStore,
    files: &Files,
    document: &Document,
    state_root: &str,
    previous: Option<&Lock>,
) -> Result<Outcome, String> {
    let through = |error: RemoteError| format!("{error}; {CONNECTED}");
    let key = &document.id.key().to_string();
    let client = remote::connected(document.remote.as_deref(), key).map_err(through)?;
    let head = head(&client, key).map_err(through)?;
    let base = match previous {
        Some(lock) => mark(
            &json!(lock.publish_sequence.get()),
            &json!(lock.state_root.to_string()),
        ),
        None => mark(&json!(0), &Value::Null),
    };
    let at_head = if head.is_null() {
        previous.is_none()
    } else {
        mark(&head["publishSequence"], &head["stateRoot"]) == base
    };
    if at_head && head["stateRoot"] == state_root {
        return Ok(Outcome::Unchanged {
            sequence: head["publishSequence"].as_u64().unwrap_or(0),
            state_root: state_root.to_owned(),
        });
    }
    let base_files = match previous {
        None => BTreeMap::new(),
        Some(_) if at_head => head_digests(&head),
        Some(lock) => match history_digests(root, lock) {
            Some(digests) => digests,
            None => state_digests(&client, lock).map_err(through)?,
        },
    };
    let delta = delta(files, &base_files);
    let retry = retry_key(key, &base["stateRoot"], &delta);
    let mut body = json!({ "documentKey": key, "base": base, "retryKey": retry });
    for (name, value) in delta.as_object().into_iter().flatten() {
        body[name] = value.clone();
    }
    let answer = client
        .post("publish", &body)
        .map_err(|error| refused(&error, &base))?;
    applied(store, files, &answer, previous)
}

/// The remote's head: `null`, or its publish sequence, state root, lock and file digests.
fn head(client: &Client, key: &str) -> Result<Value, RemoteError> {
    match client.get("head") {
        Ok(answer) => Ok(answer["head"].clone()),
        Err(RemoteError::Refused { refusal, .. }) if refusal.known() == Some("not_found") => {
            Err(RemoteError::NotHeld {
                key: key.to_owned(),
            })
        }
        Err(error) => Err(error),
    }
}

/// The digest of each file the head holds, by its path from the top of the tree.
fn head_digests(head: &Value) -> BTreeMap<String, String> {
    head["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|file| {
            Some((
                file["path"].as_str()?.to_owned(),
                file["sha256"].as_str()?.to_owned(),
            ))
        })
        .collect()
}

/// The digest of each file of the earliest commit that holds `lock`, none where no commit of the
/// history holds it.
fn history_digests(root: &Path, lock: &Lock) -> Option<BTreeMap<String, String>> {
    let git = GitCommand::default();
    for commit in git.lock_history(root).ok()? {
        let Ok((Some(bytes), files)) = git.lock_and_spec_at(root, &commit) else {
            continue;
        };
        let held = Lock::read(&bytes).ok()?;
        if held.publish_sequence == lock.publish_sequence && held.state_root == lock.state_root {
            return Some(
                files
                    .iter()
                    .map(|(path, bytes)| (format!("spec/{}", path.as_str()), sha256(bytes)))
                    .collect(),
            );
        }
    }
    None
}

/// The digest of each file of the state `lock` names, as the remote gives it.
fn state_digests(client: &Client, lock: &Lock) -> Result<BTreeMap<String, String>, RemoteError> {
    let sequence = lock.publish_sequence.get();
    let missing = || RemoteError::StateNotHeld {
        sequence,
        state_root: lock.state_root.to_string(),
    };
    let state = match client.get(&format!("states/{sequence}")) {
        Err(RemoteError::Refused { refusal, .. }) if refusal.known() == Some("not_found") => {
            return Err(missing());
        }
        answer => answer?,
    };
    if state["lock"]["stateRoot"] != lock.state_root.to_string() {
        return Err(missing());
    }
    Ok(state["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|file| {
            Some((
                file["path"].as_str()?.to_owned(),
                sha256(file["text"].as_str()?.as_bytes()),
            ))
        })
        .collect())
}

/// The change from `base` to `files`, `spec/labels.yaml` left out: each file added or changed
/// with its text, in path order, and each removed by its path.
fn delta(files: &Files, base: &BTreeMap<String, String>) -> Value {
    let labels = format!("spec/{LABELS}");
    let mut added = Vec::new();
    let mut changed = Vec::new();
    for (path, bytes) in files {
        let path = format!("spec/{}", path.as_str());
        if path == labels {
            continue;
        }
        let text = String::from_utf8_lossy(bytes);
        match base.get(&path) {
            None => added.push(json!({ "path": path, "text": text })),
            Some(digest) if *digest != sha256(bytes) => {
                changed.push(json!({ "path": path, "text": text }));
            }
            Some(_) => {}
        }
    }
    let removed: Vec<&String> = base
        .keys()
        .filter(|path| **path != labels)
        .filter(|path| {
            path.strip_prefix("spec/")
                .is_none_or(|under| !files.contains_key(&SpecPath::new(under)))
        })
        .collect();
    json!({ "added": added, "changed": changed, "removed": removed })
}

/// The retry key of a publish: the SHA-256 digest of the RFC 8785 canonical JSON of the document
/// key, the base's state root and the delta.
fn retry_key(key: &str, base_root: &Value, delta: &Value) -> String {
    sha256(&canonical_bytes(&json!([key, base_root, delta])))
}

fn sha256(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// A refusal of the publish, as the error that stops it.
fn refused(error: &RemoteError, base: &Value) -> String {
    let RemoteError::Refused { refusal, .. } = error else {
        return format!("{error}; {CONNECTED}");
    };
    match refusal.known() {
        Some("conflict") if refusal.problem["head"] != *base => {
            let head = &refusal.problem["head"];
            format!(
                "the remote's head is at publish sequence {}, state root {}: `loft pull` brings \
                 the tree to the head",
                head["publishSequence"],
                head["stateRoot"].as_str().unwrap_or("none")
            )
        }
        Some("invalid_request") => {
            let paths: Vec<&str> = refusal.problem["paths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            if paths.is_empty() {
                error.to_string()
            } else {
                format!("{error}: {}", paths.join(", "))
            }
        }
        _ => error.to_string(),
    }
}

/// The remote's answer applied: nothing for `unchanged`; otherwise the returned files over the
/// tree, whose roots must agree with the returned lock, written with the lock in one save,
/// unless `loft.lock.json` already holds that lock.
fn applied(
    store: &FileStore,
    files: &Files,
    answer: &Value,
    previous: Option<&Lock>,
) -> Result<Outcome, String> {
    let lock = Lock::read(answer["lock"].to_string().as_bytes())
        .map_err(|error| format!("the remote returned a lock that does not read: {error}"))?;
    if answer["outcome"] == "unchanged" {
        return Ok(Outcome::Unchanged {
            sequence: lock.publish_sequence.get(),
            state_root: lock.state_root.to_string(),
        });
    }
    let mut returned = Files::new();
    for file in answer["files"].as_array().into_iter().flatten() {
        let (Some(path), Some(text)) = (file["path"].as_str(), file["text"].as_str()) else {
            return Err("the remote returned a file without its path or text".to_owned());
        };
        let under = path
            .strip_prefix("spec/")
            .ok_or_else(|| format!("the remote returned a file outside spec/: {path}"))?;
        returned.insert(SpecPath::new(under), text.as_bytes().to_vec());
    }
    let mut tree = files.clone();
    tree.extend(returned.clone());
    let read = ReadTree::read(&tree)
        .map_err(|_| "the files the remote returned do not read with the tree".to_owned())?;
    verify(&read, &lock).map_err(|unverified| match unverified {
        Unverified::Mismatch { root, lock, tree } => {
            format!("the {root} the remote returned is {lock}, but the tree it returns has {tree}")
        }
        Unverified::Invalid(_) => "the tree the remote returns does not validate".to_owned(),
    })?;
    let held = previous.is_some_and(|previous| previous.to_bytes() == lock.to_bytes());
    if !held {
        store
            .publish(returned, &lock)
            .map_err(|error| error.to_string())?;
    }
    let labels = answer["labels"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|given| {
            Some((
                given["key"].as_str()?.to_owned(),
                given["label"].as_str()?.to_owned(),
            ))
        })
        .collect();
    Ok(Outcome::Published { lock, labels })
}
