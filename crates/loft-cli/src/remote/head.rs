//! A document's head as the remote names it: its publish sequence and state root.

use loftspec::publish::Lock;
use serde_json::{Value, json};

use crate::remote::{Client, RemoteError};

/// The remote's head of the document `key`: `null`, or its publish sequence, state root, lock
/// and file digests.
///
/// # Errors
///
/// The remote cannot be reached, refuses, or holds no document `key`.
pub fn fetch(client: &Client, key: &str) -> Result<Value, RemoteError> {
    match client.get("head") {
        Ok(mut answer) => Ok(answer["head"].take()),
        Err(error) if error.refusal("not_found").is_some() => Err(RemoteError::NotHeld {
            key: key.to_owned(),
        }),
        Err(error) => Err(error),
    }
}

/// The head of a document by its publish sequence and state root: `0` and none for a document
/// that holds no published state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    pub sequence: u64,
    pub state_root: Option<String>,
}

impl Head {
    /// The head that `lock` names, or none without a lock.
    pub fn of_lock(lock: Option<&Lock>) -> Self {
        lock.map_or(Self::none(), |lock| Self {
            sequence: lock.publish_sequence.get(),
            state_root: Some(lock.state_root.to_string()),
        })
    }

    pub const fn none() -> Self {
        Self {
            sequence: 0,
            state_root: None,
        }
    }

    /// The head that `value` names by `publishSequence` and `stateRoot`, if it names one.
    pub fn read(value: &Value) -> Option<Self> {
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
