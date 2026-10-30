//! Published states for the remote's tests: the Reading List published once, then changed and
//! published again, each state as a remote holds it.

use std::collections::BTreeMap;

use loftspec::codec::{Files, SpecPath};
use loftspec::publish::{self, Lock, Made, Publication, PublishedAt, ReadTree};
use serde_json::Value;
use software_plant::remote::StoredState;

use super::{EPOCH, reading_list};

/// A document's published history: its key and each state, from publish sequence 1.
pub struct History {
    pub key: String,
    pub states: Vec<StoredState>,
}

/// `files` by their paths from the top of the tree, as the wire names them.
pub fn wire(files: &Files) -> BTreeMap<String, String> {
    files
        .iter()
        .map(|(path, bytes)| {
            (
                format!("spec/{}", path.as_str()),
                String::from_utf8(bytes.clone()).unwrap(),
            )
        })
        .collect()
}

/// `files` from their paths on the wire.
pub fn spec(files: &BTreeMap<String, String>) -> Files {
    files
        .iter()
        .map(|(path, text)| {
            let under = path.strip_prefix("spec/").unwrap();
            (SpecPath::new(under), text.clone().into_bytes())
        })
        .collect()
}

/// `files` with one criterion added to the first Requirement that holds criteria, under
/// `key`, without a label.
pub fn with_criterion(files: &Files, key: &str) -> Files {
    let mut files = files.clone();
    let (path, text) = files
        .iter()
        .map(|(path, bytes)| (path.clone(), String::from_utf8(bytes.clone()).unwrap()))
        .find(|(_, text)| text.contains("    criteria:\n"))
        .expect("a Requirement with criteria");
    let added = format!("    criteria:\n      - key: {key}\n        name: \"A new criterion.\"\n");
    files.insert(
        path,
        text.replacen("    criteria:\n", &added, 1).into_bytes(),
    );
    files
}

/// The Reading List published `count` times, a criterion added before each publish after the
/// first, a minute apart from [`EPOCH`].
pub fn history(count: u64) -> History {
    let mut files = reading_list();
    let mut states = Vec::new();
    let mut previous = None;
    for sequence in 1..=count {
        if sequence > 1 {
            files = with_criterion(&files, &format!("01a18e30-0000-7000-8000-{sequence:012}"));
        }
        let (lock, published) = publish_at(&files, previous.as_ref(), EPOCH + 60 * sequence);
        files = published;
        states.push(stored(&lock, &files));
        previous = Some(lock);
    }
    let key = states[0].lock["documentKey"].as_str().unwrap().to_owned();
    History { key, states }
}

/// `files` published after `previous` at `seconds` past the Unix epoch: the publish's lock, and
/// `files` with the labels it gave.
pub fn publish_at(files: &Files, previous: Option<&Lock>, seconds: u64) -> (Lock, Files) {
    let tree = ReadTree::read(files).unwrap();
    let at = PublishedAt::from_unix_seconds(seconds).unwrap();
    let Publication::Published {
        lock,
        files: written,
        ..
    } = publish::publish(tree, None, previous, at, Made::NotAsRemote).unwrap()
    else {
        panic!("the change publishes");
    };
    let mut files = files.clone();
    files.extend(written);
    (lock, files)
}

/// The state `lock` names, whose files are `files`, as a remote holds it.
pub fn stored(lock: &Lock, files: &Files) -> StoredState {
    StoredState {
        lock: serde_json::from_slice::<Value>(&lock.to_bytes()).unwrap(),
        files: wire(files),
    }
}
