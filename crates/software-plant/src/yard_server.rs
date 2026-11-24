//! The Yard Server simulator: an HTTP endpoint on the loopback interface that takes batches of
//! versions as Yard Server does. A test points the binary under test at it with
//! `LOFT_YARD_SERVER_ENDPOINT` and `LOFT_YARD_SERVER_TOKEN`, scripts how it answers, and reads the
//! requests it received and the records it stored. It holds each version once, by its key, in
//! memory, in the order it first received them.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::http::{Answer, Request, Responder, Response, Server, refusing_url};
use crate::remote::problem::problem;

/// The path that takes a batch of versions, under the server's address.
pub const BATCHES: &str = "/batches";

/// The format a batch of versions names.
pub const FORMAT: &str = "yardlog-batch/1";

/// The most records a batch may hold.
pub const MOST_RECORDS: usize = 16;

/// The bearer token the simulator admits.
pub const TOKEN: &str = "simulated-yard-server-token";

/// What the simulator does once a batch is stored, or in place of storing it.
#[derive(Clone, Debug)]
enum Behavior {
    /// As Yard Server's contract states.
    Accepting,
    /// Stores the batch and closes the connection before a byte of its answer.
    HangingUp,
    /// Stores the batch and answers once the delay has passed.
    Late(Duration),
    /// Answers every request with this status, as a problem document.
    Failing(u16),
    /// Accepts each connection and never answers.
    Silent,
}

/// A running Yard Server simulator.
pub struct YardServer {
    server: Server,
    stored: Arc<Mutex<Vec<Value>>>,
}

impl YardServer {
    /// A server that takes batches as Yard Server's contract states, admitting [`TOKEN`].
    #[must_use]
    pub fn accepting() -> Self {
        Self::start(Behavior::Accepting)
    }

    /// A server that stores each batch it takes, then closes the connection unanswered, as a
    /// server whose answer is lost does.
    #[must_use]
    pub fn hanging_up() -> Self {
        Self::start(Behavior::HangingUp)
    }

    /// A server that stores each batch it takes and answers it only after `delay`.
    #[must_use]
    pub fn late(delay: Duration) -> Self {
        Self::start(Behavior::Late(delay))
    }

    /// A server that answers every request with `status`, as a problem document.
    #[must_use]
    pub fn failing(status: u16) -> Self {
        Self::start(Behavior::Failing(status))
    }

    /// A server that accepts each connection and never answers.
    #[must_use]
    pub fn silent() -> Self {
        Self::start(Behavior::Silent)
    }

    /// The address of a server that refuses every connection.
    #[must_use]
    pub fn refusing() -> String {
        refusing_url()
    }

    fn start(behavior: Behavior) -> Self {
        let stored = Arc::new(Mutex::new(Vec::new()));
        let server = Server::start(Batches {
            behavior,
            stored: Arc::clone(&stored),
        });
        Self { server, stored }
    }

    /// The server's base address, for `LOFT_YARD_SERVER_ENDPOINT`.
    #[must_use]
    pub fn url(&self) -> String {
        self.server.url()
    }

    /// Every request received, answered or not, in the order received.
    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        self.server.requests()
    }

    /// The records the server holds, one for each version, in the order it first received them.
    #[must_use]
    pub fn records(&self) -> Vec<Value> {
        self.stored
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

struct Batches {
    behavior: Behavior,
    stored: Arc<Mutex<Vec<Value>>>,
}

impl Responder for Batches {
    fn answer(&mut self, request: &Request) -> Answer {
        match self.behavior {
            Behavior::Silent => return Answer::Hold,
            Behavior::Failing(status) => return failure(status).into(),
            _ => {}
        }
        let records = match admit(request) {
            Ok(records) => records,
            Err(refusal) => return refusal.into(),
        };
        let echo = self.store(records);
        let response = Response::json(200, &json!({ "format": FORMAT, "records": echo }));
        match self.behavior {
            Behavior::HangingUp => Answer::HangUp,
            Behavior::Late(after) => Answer::Late { response, after },
            _ => response.into(),
        }
    }
}

impl Batches {
    /// Stores each record whose version the server does not hold, and gives the record it holds
    /// for each, in the order of `records`.
    fn store(&self, records: Vec<Value>) -> Vec<Value> {
        let mut stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        records
            .into_iter()
            .map(|record| {
                let key = version_key(&record);
                if let Some(held) = stored.iter().find(|held| version_key(held) == key) {
                    held.clone()
                } else {
                    stored.push(record.clone());
                    record
                }
            })
            .collect()
    }
}

/// The records of `request`, or the refusal it is answered with.
fn admit(request: &Request) -> Result<Vec<Value>, Response> {
    if request.path != BATCHES {
        return Err(refusal(404, "not_found"));
    }
    if request.method != "POST" {
        return Err(refusal(405, "method_not_allowed"));
    }
    let bearer = format!("Bearer {TOKEN}");
    if request.header("Authorization") != Some(bearer.as_str()) {
        return Err(refusal(401, "unauthenticated"));
    }
    let json = request
        .header("Content-Type")
        .and_then(|value| value.split(';').next())
        .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"));
    if !json {
        return Err(refusal(415, "unsupported_media_type"));
    }
    records(&request.body).ok_or_else(|| refusal(400, "invalid_request"))
}

/// The records of a batch of versions, where `body` is one the server can take.
fn records(body: &[u8]) -> Option<Vec<Value>> {
    let batch: Map<String, Value> = serde_json::from_slice(body).ok()?;
    if batch.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return None;
    }
    let records = batch.get("records")?.as_array()?;
    let readable = (1..=MOST_RECORDS).contains(&records.len())
        && records.iter().all(|record| version_key(record).is_some());
    readable.then(|| records.clone())
}

/// The key of a record's version.
fn version_key(record: &Value) -> Option<&str> {
    record.get("version")?.get("key")?.as_str()
}

fn refusal(status: u16, code: &str) -> Response {
    problem(status, code, Map::new())
}

/// The problem document of an error scenario at `status`.
fn failure(status: u16) -> Response {
    let code = if status >= 500 {
        "internal_error"
    } else {
        "invalid_request"
    };
    refusal(status, code)
}
