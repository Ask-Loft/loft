//! Acts: every write, answer, decision and change of state is one, recording when it was made, at
//! which revision of Yard Log, and by whom.

use std::fmt;

/// A time in UTC to the second, written as `YYYY-MM-DDTHH:MM:SSZ`.
pub use loft_common::time::{Time, TimeError};

/// The host an agent runs in.
pub const CLAUDE_CODE: &str = "claude-code";

/// When an Act was made, at which revision, and by whom.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Act {
    pub at: Time,
    pub revision: u64,
    pub actor: Actor,
}

/// Who made an Act: an agent, or the person.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Actor {
    Agent(Agent),
    /// The person, as git names them for the repository.
    Person {
        name: String,
        email: String,
    },
}

/// An agent: a host's session and, for a subagent, its agent id and type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Agent {
    pub host: String,
    pub session: String,
    pub subagent: Option<Subagent>,
}

/// A subagent of a session.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Subagent {
    pub id: String,
    pub kind: String,
}

impl Agent {
    /// The main agent of a Claude Code session.
    #[must_use]
    pub fn main(session: &str) -> Self {
        Self {
            host: CLAUDE_CODE.to_owned(),
            session: session.to_owned(),
            subagent: None,
        }
    }

    /// A subagent of a Claude Code session.
    #[must_use]
    pub fn sub(session: &str, id: &str, kind: &str) -> Self {
        Self {
            subagent: Some(Subagent {
                id: id.to_owned(),
                kind: kind.to_owned(),
            }),
            ..Self::main(session)
        }
    }

    /// Whether this is a session's main agent.
    #[must_use]
    pub const fn is_main(&self) -> bool {
        self.subagent.is_none()
    }

    /// Whether `other` is this agent, whatever type either names.
    #[must_use]
    pub fn same(&self, other: &Self) -> bool {
        self.host == other.host
            && self.session == other.session
            && self.subagent.as_ref().map(|sub| &sub.id)
                == other.subagent.as_ref().map(|sub| &sub.id)
    }
}

impl Actor {
    /// The agent, where an agent made the Act.
    #[must_use]
    pub const fn agent(&self) -> Option<&Agent> {
        match self {
            Self::Agent(agent) => Some(agent),
            Self::Person { .. } => None,
        }
    }
}

impl fmt::Display for Actor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Agent(Agent {
                session,
                subagent: None,
                ..
            }) => write!(formatter, "session {session}"),
            Self::Agent(Agent {
                session,
                subagent: Some(sub),
                ..
            }) => write!(formatter, "{} {} of session {session}", sub.kind, sub.id),
            Self::Person { name, email } => write!(formatter, "{name} <{email}>"),
        }
    }
}

/// Where the person answered or decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Channel {
    Page,
    Terminal,
}

impl Channel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Terminal => "terminal",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "page" => Some(Self::Page),
            "terminal" => Some(Self::Terminal),
            _ => None,
        }
    }
}
