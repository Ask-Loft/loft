//! The agent that runs a `loft yard` command, and who it counts as: a session that continues
//! another, as after `/clear`, counts as the first session of its chain for the submissions it
//! sent and the revision it last read, while every Act keeps the session id that made it. The
//! continuations and the revisions last read are the machine's, kept in `~/.yard/`.

use serde_json::{Map, Value, json};
use yardlog::identity::session_key;
use yardlog::{Actor, Agent, YardLogKey};
use yardlog_store::home::{Home, HomeFile};

use super::args;
use super::failure::Failure;
use super::home_json;

/// The longest chain of continuations followed, against a cycle in a file edited by hand.
const LONGEST_CHAIN: usize = 64;

/// The agent that runs a command, where it names one.
pub struct Caller {
    pub agent: Option<Agent>,
}

impl Caller {
    pub fn new(args: &args::Runner) -> Self {
        let agent = args.session.as_deref().map(|session| {
            match (args.agent.as_deref(), args.agent_type.as_deref()) {
                (Some(id), Some(kind)) => Agent::sub_in(&args.host, session, id, kind),
                _ => Agent::main_in(&args.host, session),
            }
        });
        Self { agent }
    }

    /// The agent, which a command that changes Yard Log always has.
    pub fn agent(&self) -> &Agent {
        self.agent.as_ref().expect("clap requires `--session`")
    }
}

/// Records that `session` continues `ended`, as `loft yard continue` does; whether it changed
/// `~/.yard/continuations.json`, which it leaves as it was where it records that already.
pub fn record_continuation(
    home: &Home,
    host: &str,
    session: &str,
    ended: &str,
) -> Result<bool, Failure> {
    home_json::set(
        home,
        HomeFile::Continuations,
        &session_key(host, session),
        json!(ended),
    )
}

/// Who an agent counts as: the first session of its chain, and its subagent id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Identity {
    host: String,
    session: String,
    subagent: Option<String>,
}

impl Identity {
    /// Whether both identities refer to one host's session.
    pub fn same_session(&self, other: &Self) -> bool {
        self.host == other.host && self.session == other.session
    }

    /// Whether the agent is a subagent.
    pub const fn is_subagent(&self) -> bool {
        self.subagent.is_some()
    }

    /// The name under which the revision this agent last read is kept.
    fn reader(&self) -> String {
        match &self.subagent {
            Some(id) => format!("{}/{id}", session_key(&self.host, &self.session)),
            None => session_key(&self.host, &self.session),
        }
    }
}

/// The continuations this machine records, read once for a command.
pub struct Continuations(Map<String, Value>);

impl Continuations {
    pub fn read(home: Option<&Home>) -> Result<Self, Failure> {
        let bytes = match home {
            Some(home) => home.read(HomeFile::Continuations)?,
            None => None,
        };
        Ok(Self(home_json::object(bytes.as_deref())))
    }

    /// The first session of the chain `session` belongs to.
    fn first(&self, host: &str, session: &str) -> String {
        let mut current = session.to_owned();
        for _ in 0..LONGEST_CHAIN {
            match self
                .0
                .get(&session_key(host, &current))
                .and_then(Value::as_str)
            {
                Some(earlier) if earlier != session => earlier.clone_into(&mut current),
                _ => break,
            }
        }
        current
    }

    pub fn identity(&self, agent: &Agent) -> Identity {
        Identity {
            host: agent.host.clone(),
            session: self.first(&agent.host, &agent.session),
            subagent: agent.subagent.as_ref().map(|sub| sub.id.clone()),
        }
    }

    /// The session that continues `session`, where one does.
    pub fn continued_by(&self, host: &str, session: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(next, ended)| {
                let same_host = host_of(next) == host;
                same_host && ended.as_str() == Some(session)
            })
            .map(|(next, _)| {
                next.strip_prefix(&format!("{host}~"))
                    .unwrap_or(next.as_str())
            })
    }

    /// Who an Act's actor counts as, where an agent made it.
    pub fn identity_of(&self, actor: &Actor) -> Option<Identity> {
        actor.agent().map(|agent| self.identity(agent))
    }
}

/// The revisions each agent last read of each Yard Log on this machine.
pub struct LastRead<'a> {
    home: &'a Home,
}

impl<'a> LastRead<'a> {
    pub const fn new(home: &'a Home) -> Self {
        Self { home }
    }

    /// Only this reader's recorded revision, without inheriting another agent's progress.
    pub fn own(&self, key: YardLogKey, reader: &Identity) -> Result<Option<u64>, Failure> {
        let bytes = self.home.read(HomeFile::LastRead)?;
        let all = home_json::object(bytes.as_deref());
        Ok(all
            .get(&key.to_string())
            .and_then(|readers| readers.get(reader.reader()))
            .and_then(Value::as_u64))
    }

    /// The revision `reader` last read of Yard Log `key`, or, where it read none, the latest
    /// any agent last read of it.
    pub fn before(&self, key: YardLogKey, reader: &Identity) -> Result<Option<u64>, Failure> {
        let bytes = self.home.read(HomeFile::LastRead)?;
        let all = home_json::object(bytes.as_deref());
        let Some(Value::Object(readers)) = all.get(&key.to_string()) else {
            return Ok(None);
        };
        let own = readers.get(&reader.reader()).and_then(Value::as_u64);
        Ok(own.or_else(|| {
            readers
                .iter()
                .filter(|(name, _)| host_of(name) == reader.host)
                .filter_map(|(_, value)| value.as_u64())
                .max()
        }))
    }

    /// Records `revision` as the one `reader` last read of Yard Log `key`.
    pub fn record(&self, key: YardLogKey, reader: &Identity, revision: u64) -> Result<(), Failure> {
        let name = reader.reader();
        self.home.update(HomeFile::LastRead, |bytes| {
            let mut all = home_json::object(bytes.as_deref());
            let readers = all
                .entry(key.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if !readers.is_object() {
                *readers = Value::Object(Map::new());
            }
            if readers[&name] == json!(revision) {
                return None;
            }
            readers[&name] = json!(revision);
            Some(home_json::ordered(&all))
        })?;
        Ok(())
    }
}

/// The namespace of a stored session or reader; unqualified legacy keys belong to Claude.
fn host_of(key: &str) -> &str {
    key.split_once('~')
        .map_or(yardlog::act::CLAUDE_CODE, |(host, _)| host)
}
