//! A document's head as the remote names it: its publish sequence and state root.

use loftspec::publish::Lock;
use serde_json::{Value, json};

use crate::escaped::Escaped;
use crate::remote::visibility::Visibility;
use crate::remote::{Client, RemoteError};

/// The remote's answer to `GET <address>/head`: the head, and to whom it serves the document.
#[derive(Debug)]
pub struct Answered {
    /// `null`, or the head's publish sequence, state root, lock and file digests, which
    /// [`Head::read`] reads.
    pub head: Value,
    /// The visibility the answer names, where it names one Loft knows.
    pub visibility: Option<Visibility>,
}

/// The remote's head of the document `key`, and its visibility.
///
/// # Errors
///
/// The remote cannot be reached, refuses, holds no document `key`, or gives a head that does
/// not read.
pub fn fetch(client: &Client, key: &str) -> Result<Answered, RemoteError> {
    match client.get("head") {
        Ok(mut answer) => {
            let visibility = Visibility::of(&answer);
            let head = answer["head"].take();
            if head.is_null() || Head::read(&head).is_some() {
                Ok(Answered { head, visibility })
            } else {
                Err(RemoteError::Unreadable {
                    address: client.address().to_owned(),
                    what: "head",
                })
            }
        }
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

/// The head in words, its state root, which a remote may have given, written escaped.
impl std::fmt::Display for Head {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.state_root {
            Some(root) => write!(
                formatter,
                "publish sequence {}, state root {}",
                self.sequence,
                Escaped(root)
            ),
            None => write!(
                formatter,
                "publish sequence {}, no state root",
                self.sequence
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Head;

    #[test]
    fn a_head_s_state_root_is_written_with_its_control_characters_escaped() {
        let head = Head::read(&json!({ "publishSequence": 2, "stateRoot": "ab\u{1b}[0m\u{9b}" }));
        assert_eq!(
            head.unwrap().to_string(),
            "publish sequence 2, state root ab\\u{1b}[0m\\u{9b}"
        );
    }
}
