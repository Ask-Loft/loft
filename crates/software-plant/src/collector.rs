//! The telemetry collector simulator: an HTTP endpoint on the loopback interface that answers
//! as the collector's contract states, or fails as the collector can. A test points the binary
//! under test at it with `LOFT_TELEMETRY_ENDPOINT` and reads the events it accepted.
//!
//! The contract, which the real collector is built to and the contract suite holds both to:
//! a `POST` to `/api/telemetry` with `Content-Type: application/json` whose body is a valid
//! command event is answered `202` with an empty body; a second event with an event id already
//! received is answered `202` and recorded once; a body that is not a valid event is answered
//! `400`, one larger than 4 KiB `413`, any method but `POST` `405`, another content type `415`;
//! and no answer sets a cookie.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

use serde::Deserialize;

use crate::http::{Request, Responder, Response, Server, refusing_url};

/// The path the collector receives events at.
pub const PATH: &str = "/api/telemetry";

/// The largest event the collector reads, in bytes.
pub const LIMIT: usize = 4096;

/// How the simulator answers.
#[derive(Clone, Copy, Debug)]
enum Behaviour {
    Accepting,
    Failing(u16),
    Silent,
}

/// A running collector simulator.
pub struct Collector {
    server: Server,
    accepted: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Collector {
    /// A collector that answers as its contract states.
    #[must_use]
    pub fn accepting() -> Self {
        Self::start(Behaviour::Accepting)
    }

    /// A collector that answers every request with `status`.
    #[must_use]
    pub fn failing(status: u16) -> Self {
        Self::start(Behaviour::Failing(status))
    }

    /// A collector that accepts each connection and never answers.
    #[must_use]
    pub fn silent() -> Self {
        Self::start(Behaviour::Silent)
    }

    /// The address of a collector that refuses every connection.
    #[must_use]
    pub fn refusing() -> String {
        format!("{}{PATH}", refusing_url())
    }

    fn start(behaviour: Behaviour) -> Self {
        let accepted = Arc::new(Mutex::new(Vec::new()));
        let responder = Answering {
            behaviour,
            accepted: Arc::clone(&accepted),
            seen: BTreeSet::new(),
        };
        Self {
            server: Server::start(responder),
            accepted,
        }
    }

    /// The address events are sent to, for `LOFT_TELEMETRY_ENDPOINT`.
    #[must_use]
    pub fn url(&self) -> String {
        format!("{}{PATH}", self.server.url())
    }

    /// Each event accepted and recorded, byte for byte, in the order received.
    pub fn events(&self) -> Vec<Vec<u8>> {
        self.accepted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Every request received, answered or not.
    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        self.server.requests()
    }
}

struct Answering {
    behaviour: Behaviour,
    accepted: Arc<Mutex<Vec<Vec<u8>>>>,
    seen: BTreeSet<String>,
}

impl Responder for Answering {
    fn answer(&mut self, request: &Request) -> Option<Response> {
        match self.behaviour {
            Behaviour::Silent => None,
            Behaviour::Failing(status) => Some(Response::empty(status)),
            Behaviour::Accepting => Some(Response::empty(self.accept(request))),
        }
    }
}

impl Answering {
    /// The status the contract gives `request`, recording a new valid event.
    fn accept(&mut self, request: &Request) -> u16 {
        if request.path != PATH {
            return 404;
        }
        if request.method != "POST" {
            return 405;
        }
        let json = request
            .header("content-type")
            .and_then(|value| value.split(';').next())
            .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"));
        if !json {
            return 415;
        }
        if request.body.len() > LIMIT {
            return 413;
        }
        let Some(event) = valid(&request.body) else {
            return 400;
        };
        if self.seen.insert(event) {
            self.accepted
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(request.body.clone());
        }
        202
    }
}

/// The event id of `body`, if it is a valid command event.
fn valid(body: &[u8]) -> Option<String> {
    let event: Event = serde_json::from_slice(body).ok()?;
    let checks = [
        event.schema == "loft-telemetry-event/2",
        uuid_v4(&event.id),
        uuid_v4(&event.installation),
        event.project.as_deref().is_none_or(uuid_v4),
        event.branch.as_deref().is_none_or(|branch| {
            branch.len() == 16
                && branch
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        }),
        hour(&event.time),
        !event.version.is_empty() && event.version.len() <= 32,
        !event.outcome.is_empty() && event.outcome.len() <= 32,
    ];
    checks.iter().all(|check| *check).then_some(event.id)
}

/// Whether `text` is a random UUID, version 4, in lower case.
fn uuid_v4(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(at, byte)| match at {
            8 | 13 | 18 | 23 => *byte == b'-',
            14 => *byte == b'4',
            19 => matches!(byte, b'8' | b'9' | b'a' | b'b'),
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(byte),
        })
}

/// Whether `text` is an hour in UTC: `YYYY-MM-DDTHH:00:00Z`.
fn hour(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 20
        && text.ends_with(":00:00Z")
        && bytes.iter().enumerate().all(|(at, byte)| match at {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
}

/// The command event, as the collector reads it: every field closed, none beyond these.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[expect(dead_code, reason = "reading a field is how serde checks its value")]
struct Event {
    schema: String,
    #[serde(rename = "eventId")]
    id: String,
    version: String,
    os: Os,
    arch: Arch,
    installed_by: InstalledBy,
    time: String,
    installation: String,
    project: Option<String>,
    branch: Option<String>,
    mode: Mode,
    agent: Agent,
    command: CommandName,
    outcome: String,
    error_kinds: Vec<ErrorKind>,
    duration: Duration,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Os {
    Linux,
    Macos,
    Windows,
    Other,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Arch {
    X86_64,
    Aarch64,
    Other,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum InstalledBy {
    Npm,
    Cargo,
    ReleaseBinary,
    Unknown,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Internal,
    Ci,
    Agent,
    Interactive,
    Unknown,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Agent {
    Claude,
    ClaudeCode,
    Chatgpt,
    ChatgptCodex,
    Unknown,
}

#[derive(Deserialize)]
enum CommandName {
    #[serde(rename = "check")]
    Check,
    #[serde(rename = "find")]
    Find,
    #[serde(rename = "fmt")]
    Fmt,
    #[serde(rename = "import v1")]
    ImportV1,
    #[serde(rename = "init")]
    Init,
    #[serde(rename = "publish")]
    Publish,
    #[serde(rename = "reconcile")]
    Reconcile,
    #[serde(rename = "render")]
    Render,
    #[serde(rename = "skill install")]
    SkillInstall,
    #[serde(rename = "verify")]
    Verify,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ErrorKind {
    Store,
    Read,
    Validation,
    LintConfig,
    Refused,
    Integrity,
    Lock,
    Install,
    Accept,
}

#[derive(Deserialize)]
enum Duration {
    #[serde(rename = "under-100ms")]
    Under100Ms,
    #[serde(rename = "under-1s")]
    Under1S,
    #[serde(rename = "under-10s")]
    Under10S,
    #[serde(rename = "under-60s")]
    Under60S,
    #[serde(rename = "60s-or-more")]
    SixtySecondsOrMore,
}
