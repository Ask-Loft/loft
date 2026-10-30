//! The publish of a connected document, through its remote: `loft publish` asks for the remote's
//! head, takes the base the lock names from the head, the history or the remote, sends the
//! change from it as a delta under a retry key, and applies the labels, files and lock the remote
//! returns once the roots it recomputes agree with that lock. The publisher never runs here as
//! the document's remote.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::path::Path;

use loftspec::canonical::{Digest, canonical_bytes};
use loftspec::codec::{Files, SpecPath};
use loftspec::document::Document;
use loftspec::publish::{Lock, ReadTree, Unverified, verify};
use loftspec_store::FileStore;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::remote::head::Head;
use crate::remote::{self, Client, RemoteError, history};

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
    /// There was nothing to publish: the head's lock.
    Unchanged(Lock),
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
    files: Files,
    document: &Document,
    state_root: &Digest,
    previous: Option<&Lock>,
) -> Result<Outcome, String> {
    let key = &document.id.key().to_string();
    let client = remote::connected(document.remote.as_deref(), key).map_err(through)?;
    let head = head(&client, key).map_err(through)?;
    let base = Head::of_lock(previous);
    let at_head = Head::read(&head).unwrap_or_else(Head::none) == base;
    if let Some(lock) = previous
        && at_head
        && lock.state_root == *state_root
    {
        return Ok(Outcome::Unchanged(lock.clone()));
    }
    let base_files = match previous {
        None => BTreeMap::new(),
        Some(_) if at_head => pairs(&head["files"], "path", "sha256")
            .map(|(path, digest)| (path.to_owned(), digest.to_owned()))
            .collect(),
        Some(lock) => match history_digests(root, lock) {
            Some(digests) => digests,
            None => state_digests(&client, lock).map_err(through)?,
        },
    };
    let mut body = delta(&files, &base_files);
    let retry = retry_key(key, base.state_root.as_deref(), &body);
    body["documentKey"] = json!(key);
    body["base"] = base.json();
    body["retryKey"] = json!(retry);
    let answer = client
        .post("publish", &body)
        .map_err(|error| refused(&error, &base))?;
    applied(store, files, &answer, previous, &base)
}

/// A remote's error as the error that stops the publish.
fn through(error: impl Display) -> String {
    format!("{error}; {CONNECTED}")
}

/// The text members `a` and `b` of each object of `list` that has both.
fn pairs<'a>(
    list: &'a Value,
    a: &'a str,
    b: &'a str,
) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(move |item| Some((item[a].as_str()?, item[b].as_str()?)))
}

/// The remote's head: `null`, or its publish sequence, state root, lock and file digests.
fn head(client: &Client, key: &str) -> Result<Value, RemoteError> {
    match client.get("head") {
        Ok(mut answer) => Ok(answer["head"].take()),
        Err(error) if error.refusal("not_found").is_some() => Err(RemoteError::NotHeld {
            key: key.to_owned(),
        }),
        Err(error) => Err(error),
    }
}

/// The digest of each file of the state `lock` names, as the earliest commit that holds its
/// publish sequence holds it; none where no commit of the history holds that state, or its files
/// do not verify against its lock.
fn history_digests(root: &Path, lock: &Lock) -> Option<BTreeMap<String, String>> {
    let state = history::earliest(root)
        .ok()?
        .remove(&lock.publish_sequence.get())?;
    (state.lock.state_root == lock.state_root && state.verifies()).then(|| {
        state
            .files
            .iter()
            .map(|(path, bytes)| (format!("spec/{}", path.as_str()), sha256(bytes)))
            .collect()
    })
}

/// The digest of each file of the state `lock` names, as the remote gives it.
fn state_digests(client: &Client, lock: &Lock) -> Result<BTreeMap<String, String>, RemoteError> {
    let sequence = lock.publish_sequence.get();
    let missing = || RemoteError::StateNotHeld {
        sequence,
        state_root: lock.state_root.to_string(),
    };
    let state = client.get(&format!("states/{sequence}")).map_err(|error| {
        match error.refusal("not_found") {
            Some(_) => missing(),
            None => error,
        }
    })?;
    if state["lock"]["stateRoot"] != lock.state_root.to_string() {
        return Err(missing());
    }
    Ok(pairs(&state["files"], "path", "text")
        .map(|(path, text)| (path.to_owned(), sha256(text.as_bytes())))
        .collect())
}

/// The change from `base` to `files`, `spec/labels.yaml` left out: each file added or changed
/// with its text, in path order, and each file under `spec/` removed by its path.
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
                .is_some_and(|under| !files.contains_key(&SpecPath::new(under)))
        })
        .collect();
    json!({ "added": added, "changed": changed, "removed": removed })
}

/// The retry key of a publish: the SHA-256 digest of the RFC 8785 canonical JSON of the document
/// key, the base's state root and the delta.
fn retry_key(key: &str, base_root: Option<&str>, delta: &Value) -> String {
    sha256(&canonical_bytes(&(key, base_root, delta)))
}

/// The SHA-256 digest of `bytes`, in lowercase hex.
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// A refusal of the publish, as the error that stops it. A conflict is read by the head it
/// names, by publish sequence and state root alone.
fn refused(error: &RemoteError, base: &Head) -> String {
    let RemoteError::Refused { refusal, .. } = error else {
        return through(error);
    };
    match refusal.known() {
        Some("conflict") if Head::read(&refusal.problem["head"]).as_ref() != Some(base) => {
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

/// The remote's answer applied: nothing for `unchanged`, or for a replay of one, whose lock is
/// the base itself; otherwise the returned files over the tree, whose roots must agree with the
/// returned lock, written with the lock in one save, unless `loft.lock.json` already holds that
/// lock.
fn applied(
    store: &FileStore,
    mut tree: Files,
    answer: &Value,
    previous: Option<&Lock>,
    base: &Head,
) -> Result<Outcome, String> {
    let lock = Lock::read(answer["lock"].to_string().as_bytes())
        .map_err(|error| format!("the remote returned a lock that does not read: {error}"))?;
    if answer["outcome"] == "unchanged" || Head::of_lock(Some(&lock)) == *base {
        return Ok(Outcome::Unchanged(lock));
    }
    let mut returned = Files::new();
    for file in answer["files"].as_array().into_iter().flatten() {
        let (Some(path), Some(text)) = (file["path"].as_str(), file["text"].as_str()) else {
            return Err("the remote returned a file without its path or text".to_owned());
        };
        let under = path
            .strip_prefix("spec/")
            .filter(|under| {
                under
                    .split('/')
                    .all(|segment| !matches!(segment, "" | "." | ".."))
            })
            .ok_or_else(|| format!("the remote returned a file outside spec/: {path}"))?;
        returned.insert(SpecPath::new(under), text.as_bytes().to_vec());
    }
    tree.extend(returned.clone());
    let read = ReadTree::read(&tree)
        .map_err(|_| "the files the remote returned do not read with the tree".to_owned())?;
    verify(&read, &lock).map_err(|unverified| match unverified {
        Unverified::Mismatch { root, lock, tree } => {
            format!("the {root} the remote returned is {lock}, but the tree it returns has {tree}")
        }
        Unverified::Invalid(_) => "the tree the remote returns does not validate".to_owned(),
    })?;
    if previous != Some(&lock) {
        store
            .publish(returned, &lock)
            .map_err(|error| error.to_string())?;
    }
    let labels = pairs(&answer["labels"], "key", "label")
        .map(|(key, label)| (key.to_owned(), label.to_owned()))
        .collect();
    Ok(Outcome::Published { lock, labels })
}
