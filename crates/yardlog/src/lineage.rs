//! The chains of continuations a machine records: a session that continues another, as after
//! `/clear` or a resume, counts as the first session of its chain wherever Yard compares agents,
//! for the submissions it sent, the revision it last read, the stages it owns and the tasks
//! assigned to it. Every Act still keeps the session id that made it.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::Agent;
use crate::identity::{SessionKey, session_key};

/// The longest chain of continuations followed, against a cycle in a file edited by hand.
const LONGEST_CHAIN: usize = 64;

/// Each continuing session, by its session key, and the session it continues.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lineage(BTreeMap<String, String>);

impl Lineage {
    /// The lineage `~/.yard/continuations.json` holds: a JSON object of each continuing session's
    /// key and the session it continues. Bytes that hold no such object hold no continuation,
    /// and an entry whose value is not text is passed over.
    #[must_use]
    pub fn read(bytes: Option<&[u8]>) -> Self {
        let Some(Value::Object(links)) = bytes.and_then(|bytes| serde_json::from_slice(bytes).ok())
        else {
            return Self::default();
        };
        Self(
            links
                .into_iter()
                .filter_map(|(next, ended)| Some((next, ended.as_str()?.to_owned())))
                .collect(),
        )
    }

    /// The first session of the chain the host's `session` belongs to.
    #[must_use]
    pub fn first(&self, host: &str, session: &str) -> String {
        let mut current = session.to_owned();
        for _ in 0..LONGEST_CHAIN {
            match self.0.get(&session_key(host, &current)) {
                Some(earlier) if earlier != session => earlier.clone_into(&mut current),
                _ => break,
            }
        }
        current
    }

    /// The session that continues the host's `session`, where one does.
    #[must_use]
    pub fn continued_by(&self, host: &str, session: &str) -> Option<String> {
        self.0
            .iter()
            .filter(|(_, ended)| ended.as_str() == session)
            .map(|(next, _)| SessionKey::parse(next))
            .find(|next| next.host() == host)
            .map(|next| next.session().to_owned())
    }

    /// Whether `left` and `right` are one agent: of one host, of sessions of one chain, and the
    /// same subagent or both the main agent, whatever type either names.
    #[must_use]
    pub fn same(&self, left: &Agent, right: &Agent) -> bool {
        left.host == right.host
            && left.subagent.as_ref().map(|sub| &sub.id)
                == right.subagent.as_ref().map(|sub| &sub.id)
            && (left.session == right.session
                || self.first(&left.host, &left.session) == self.first(&right.host, &right.session))
    }
}
