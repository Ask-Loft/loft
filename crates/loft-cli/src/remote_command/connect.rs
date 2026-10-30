//! The connect of `loft remote set`: every state the repository has published is read from its
//! history before the remote is asked anything; the remote creates the document under its key,
//! or answers that it holds it already; and the history is imported where the remote holds no
//! published state of it. Only then does the manifest name the remote.

use std::collections::BTreeMap;
use std::path::Path;

use loftspec::codec::Files;
use loftspec::document::Document;
use loftspec::publish::{Lock, ReadTree, verify};
use loftspec_store::repository::GitCommand;
use serde_json::{Value, json};

use crate::remote::{Client, RemoteError};
use crate::stop::Stop;

/// A published state as the history holds it: its lock's bytes and its files under `spec/`.
pub struct Published {
    lock: Vec<u8>,
    files: Files,
}

/// The head of a document by its publish sequence and state root: `0` and none for a document
/// that holds no published state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    pub sequence: u64,
    pub state_root: Option<String>,
}

impl Head {
    fn of_lock(lock: Option<&Lock>) -> Self {
        lock.map_or(Self::none(), |lock| Self {
            sequence: lock.publish_sequence.get(),
            state_root: Some(lock.state_root.to_string()),
        })
    }

    fn none() -> Self {
        Self {
            sequence: 0,
            state_root: None,
        }
    }

    /// The head that `value` names by `publishSequence` and `stateRoot`, if it names one.
    fn read(value: &Value) -> Option<Self> {
        let state_root = match &value["stateRoot"] {
            Value::Null => None,
            Value::String(root) => Some(root.clone()),
            _ => return None,
        };
        Some(Self {
            sequence: value["publishSequence"].as_u64()?,
            state_root,
        })
    }

    pub fn json(&self) -> Value {
        json!({ "publishSequence": self.sequence, "stateRoot": self.state_root })
    }
}

impl std::fmt::Display for Head {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.state_root {
            Some(root) => write!(
                formatter,
                "publish sequence {}, state root {root}",
                self.sequence
            ),
            None => write!(
                formatter,
                "publish sequence {}, no state root",
                self.sequence
            ),
        }
    }
}

/// Every state the repository has published, from publish sequence 1 to that of `lock`, each as
/// the earliest commit that holds its lock holds it, its roots checked against its lock.
///
/// # Errors
///
/// The tree is not in a git repository, a publish sequence's lock is held by no commit, or a
/// state's files do not verify against its lock.
pub fn history(root: &Path, lock: &Lock) -> Result<Vec<Published>, Stop> {
    let git = GitCommand::default();
    git.prefix(root)?;
    let mut earliest: BTreeMap<u64, (Lock, Published)> = BTreeMap::new();
    for commit in git.lock_history(root)? {
        let (Some(bytes), files) = git.lock_and_spec_at(root, &commit)? else {
            continue;
        };
        let Ok(held) = Lock::read(&bytes) else {
            continue;
        };
        earliest
            .entry(held.publish_sequence.get())
            .or_insert((held, Published { lock: bytes, files }));
    }
    let last = lock.publish_sequence.get();
    let mut states = Vec::new();
    for sequence in 1..=last {
        let unheld = || {
            Stop::one(
                "history",
                format!("no commit of the history holds the lock of publish sequence {sequence}"),
            )
        };
        let (held, state) = earliest.remove(&sequence).ok_or_else(unheld)?;
        if sequence == last && held.state_root != lock.state_root {
            return Err(unheld());
        }
        let verified = ReadTree::read(&state.files).is_ok_and(|tree| verify(&tree, &held).is_ok());
        if !verified {
            return Err(Stop::one(
                "history",
                format!("the files of publish sequence {sequence} do not verify against its lock"),
            ));
        }
        states.push(state);
    }
    Ok(states)
}

/// What the connect found at the remote.
pub struct Connected {
    pub head: Head,
    /// Whether the history was imported.
    pub imported: bool,
}

/// Connects `document`, whose lock is `lock` and whose history is `states`, at the remote that
/// `client` reaches.
///
/// # Errors
///
/// The remote refuses the create other than as a `conflict`, holds another head than the lock's,
/// or refuses the import, or the import answers another head.
pub fn connect(
    client: &Client,
    document: &Document,
    lock: Option<&Lock>,
    states: &[Published],
) -> Result<Connected, Stop> {
    let key = document.id.key().to_string();
    let name = document
        .products
        .first()
        .map_or_else(|| key.clone(), |product| product.name.clone());
    let wanted = Head::of_lock(lock);
    let found = match client.post("create", &json!({ "documentKey": key, "name": name })) {
        Ok(_) => Head::none(),
        Err(error) => match &error {
            RemoteError::Refused { refusal, .. } if refusal.known() == Some("conflict") => {
                Head::read(&refusal.problem["head"]).ok_or_else(|| refused("create", &error))?
            }
            _ => return Err(refused("create", &error)),
        },
    };
    if found == wanted {
        return Ok(Connected {
            head: found,
            imported: false,
        });
    }
    if found.sequence != 0 || states.is_empty() {
        return Err(differs(&found, &wanted));
    }
    let body = json!({
        "documentKey": key,
        "states": states.iter().map(wire).collect::<Vec<_>>(),
    });
    let answer = client.post("import", &body).map_err(|error| {
        Stop::all(
            "remote",
            vec![
                format!("`import`: {error}"),
                "the remote holds the document with no published state: running \
                 `loft remote set` again imports into it"
                    .to_owned(),
            ],
        )
    })?;
    let head = Head::read(&answer["head"]).unwrap_or_else(Head::none);
    if head != wanted {
        return Err(differs(&head, &wanted));
    }
    Ok(Connected {
        head,
        imported: true,
    })
}

/// A state as an import sends it: its lock, and its files by their paths from the top of the
/// tree.
fn wire(state: &Published) -> Value {
    let lock = serde_json::from_slice::<Value>(&state.lock).unwrap_or(Value::Null);
    let files: Vec<Value> = state
        .files
        .iter()
        .map(|(path, bytes)| {
            json!({
                "path": format!("spec/{}", path.as_str()),
                "text": String::from_utf8_lossy(bytes),
            })
        })
        .collect();
    json!({ "lock": lock, "files": files })
}

/// The remote's refusal of `call`.
fn refused(call: &str, error: &RemoteError) -> Stop {
    Stop::one("remote", format!("`{call}`: {error}"))
}

/// The remote holds another head than the lock's.
fn differs(remote: &Head, lock: &Head) -> Stop {
    Stop::one(
        "remote",
        format!("the remote's head is at {remote}, but loft.lock.json is at {lock}"),
    )
}
