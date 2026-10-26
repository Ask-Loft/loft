//! One event the mod posts, as the page server reads it. An event names its session, the
//! directory of the agent it happened in, its time, its agent (the main agent, or a subagent by
//! its id and type) and its kind, with what that kind carries: names, ids, times and counts, and
//! never what was said.

use loftspec::yaml::Out;
use serde_json::Value;
use yardlog::Time;
use yardlog_store::session::is_session_id;

/// The four counts of a model request's tokens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tokens {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
}

impl Tokens {
    /// The counts an event's `tokens` gives: none where it gives none.
    fn of(value: &Value) -> Option<Self> {
        let count = |name: &str| value[name].as_u64().unwrap_or_default();
        value.is_object().then(|| Self {
            input: count("input"),
            cache_read: count("cacheRead"),
            cache_write: count("cacheWrite"),
            output: count("output"),
        })
    }

    pub fn add(&mut self, other: Self) {
        self.input += other.input;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
        self.output += other.output;
    }
}

/// The agent an event happened in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Who {
    Main,
    Sub { id: String, kind: String },
}

impl Who {
    fn of(value: &Value) -> Self {
        match (value["id"].as_str(), value["type"].as_str()) {
            (Some(id), kind) => Self::Sub {
                id: id.to_owned(),
                kind: kind.unwrap_or_default().to_owned(),
            },
            _ => Self::Main,
        }
    }

    pub const fn is_main(&self) -> bool {
        matches!(self, Self::Main)
    }

    /// The fields a session's file names the agent by: `agent`, and a subagent's `type`.
    pub fn fields(&self) -> Vec<(&'static str, Out)> {
        match self {
            Self::Main => vec![("agent", Out::Token("main".to_owned()))],
            Self::Sub { id, kind } => vec![
                ("agent", Out::Token(id.clone())),
                ("type", Out::Token(kind.clone())),
            ],
        }
    }
}

/// The strings of the list `value` holds; none where it holds no list.
pub fn strings(value: &Value) -> Vec<String> {
    value.as_array().map_or_else(Vec::new, |items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect()
    })
}

/// An event, read and checked.
#[derive(Debug, Clone)]
pub struct Event {
    pub session: String,
    pub dir: String,
    pub at: Time,
    pub who: Who,
    pub kind: String,
    /// The agent host the session runs in.
    pub host: String,
    /// The event as it was posted, kept whole in the event log.
    pub posted: Value,
}

/// The time an event gives, to the second: a fraction of a second is dropped.
fn time(text: &str) -> Option<Time> {
    let whole = match (text.find('.'), text.strip_suffix('Z')) {
        (Some(dot), Some(_)) => format!("{}Z", &text[..dot]),
        _ => text.to_owned(),
    };
    whole.parse().ok()
}

impl Event {
    /// Reads `posted` as an event.
    ///
    /// # Errors
    ///
    /// It names no session that can name a file, no absolute directory, no time or no kind.
    pub fn read(posted: &Value) -> Result<Self, String> {
        let session = posted["session"]
            .as_str()
            .filter(|session| is_session_id(session))
            .ok_or("the event names no session id that can name a file")?;
        let dir = posted["dir"]
            .as_str()
            .filter(|dir| std::path::Path::new(dir).is_absolute())
            .ok_or("the event names no absolute directory")?;
        let at = posted["at"]
            .as_str()
            .and_then(time)
            .ok_or("the event gives no time in UTC")?;
        let kind = posted["kind"]
            .as_str()
            .filter(|kind| !kind.is_empty())
            .ok_or("the event names no kind")?;
        let host = posted["host"].as_str().unwrap_or(yardlog::act::CLAUDE_CODE);
        if !yardlog::identity::is_host_id(host) {
            return Err("the event names no valid host identifier".to_owned());
        }
        Ok(Self {
            session: session.to_owned(),
            dir: dir.to_owned(),
            at,
            who: Who::of(&posted["agent"]),
            kind: kind.to_owned(),
            host: host.to_owned(),
            posted: posted.clone(),
        })
    }

    /// This session's storage identity, separate from its native identifier.
    pub fn session_key(&self) -> String {
        yardlog::identity::session_key(&self.host, &self.session)
    }

    /// A model request's tokens, or a compaction's own request's.
    pub fn tokens(&self) -> Option<Tokens> {
        Tokens::of(&self.posted["tokens"])
    }

    /// The model that answered a request.
    pub fn model(&self) -> String {
        self.posted["model"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned()
    }

    /// The tool call's id a tool call's start or end names.
    pub fn call(&self) -> Option<&str> {
        self.posted["call"].as_str()
    }

    /// The names of the `loft` subcommands a shell command's start runs.
    pub fn loft(&self) -> Vec<String> {
        strings(&self.posted["loft"])
    }

    /// Whether a file edit's start edits a file under the tree's `spec/`: none for any other
    /// event.
    pub fn spec(&self) -> Option<bool> {
        self.posted["spec"].as_bool()
    }

    /// Whether a tool call's end says it failed.
    pub fn failed(&self) -> bool {
        self.posted["failed"].as_bool().unwrap_or(false)
    }
}
