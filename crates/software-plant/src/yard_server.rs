//! The Yard Server simulator: an HTTP endpoint on the loopback interface that takes batches of
//! versions as Yard Server does. A test points the binary under test at it with
//! `LOFT_YARD_SERVER_ENDPOINT` and `LOFT_YARD_SERVER_TOKEN`, scripts how it answers, and reads the
//! requests it received and the records it stored. It holds each version once, by its key, in
//! memory, in the order it first received them, and answers each version of a batch with its
//! outcome.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::http::{Answer, Request, Responder, Response, Server, refusing_url};
use crate::remote::problem::{CODES, problem, refusal};

/// The path that takes a batch of versions, under the server's address.
pub const BATCHES: &str = "/batches";

/// The format a batch of versions names.
pub const FORMAT: &str = "yardlog-batch/1";

/// The format of the answer to a batch: each version's key and outcome.
pub const ANSWER_FORMAT: &str = "yardlog-answer/1";

/// The most records a batch may hold.
pub const MOST_RECORDS: usize = 16;

/// The largest body of a batch, in bytes: 1 MiB.
pub const MOST_BYTES: usize = 1024 * 1024;

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
    /// Answers every request with this status and `Retry-After`, as a problem document.
    ComeBack { status: u16, retry_after: String },
    /// Answers every batch it admits `200` with exactly this body, storing nothing.
    Answering(String),
    /// Accepts each connection and never answers.
    Silent,
}

/// A version's key and the code the simulator answers it with.
type Named = Vec<(String, String)>;

/// A running Yard Server simulator.
pub struct YardServer {
    server: Server,
    stored: Arc<Mutex<Vec<Value>>>,
}

impl YardServer {
    /// A server that takes batches as Yard Server's contract states, admitting [`TOKEN`].
    #[must_use]
    pub fn accepting() -> Self {
        Self::start(Batches::new(Behavior::Accepting))
    }

    /// A server that stores each batch it takes, then closes the connection unanswered, as a
    /// server whose answer is lost does.
    #[must_use]
    pub fn hanging_up() -> Self {
        Self::start(Batches::new(Behavior::HangingUp))
    }

    /// A server that stores each batch it takes and answers it only after `delay`.
    #[must_use]
    pub fn late(delay: Duration) -> Self {
        Self::start(Batches::new(Behavior::Late(delay)))
    }

    /// A server that answers every request with `status`, as a problem document.
    #[must_use]
    pub fn failing(status: u16) -> Self {
        Self::start(Batches::new(Behavior::Failing(status)))
    }

    /// A server that asks to come back later: it answers every request with `status`, `429`
    /// with the code `rate_limited` or `503` with `service_unavailable`, and `retry_after` as
    /// its `Retry-After`.
    #[must_use]
    pub fn coming_back(status: u16, retry_after: &str) -> Self {
        Self::start(Batches::new(Behavior::ComeBack {
            status,
            retry_after: retry_after.to_owned(),
        }))
    }

    /// A server that answers each version named in `versions` `refused` with the code beside
    /// it, and stores and answers the others as an accepting one does.
    #[must_use]
    pub fn refusing_versions(versions: &[(&str, &str)]) -> Self {
        let mut batches = Batches::new(Behavior::Accepting);
        batches.refused = named(versions);
        Self::start(batches)
    }

    /// A server that answers each version named in `versions` `later` with the code beside it,
    /// stores none of them, and stores and answers the others as an accepting one does.
    #[must_use]
    pub fn putting_off(versions: &[(&str, &str)]) -> Self {
        let mut batches = Batches::new(Behavior::Accepting);
        batches.put_off = named(versions);
        Self::start(batches)
    }

    /// A server that answers every batch it admits `200` with exactly `body`, and stores
    /// nothing: an answer that does not read, leaves a version out or names an unknown outcome.
    #[must_use]
    pub fn answering(body: &str) -> Self {
        Self::start(Batches::new(Behavior::Answering(body.to_owned())))
    }

    /// A server that accepts each connection and never answers.
    #[must_use]
    pub fn silent() -> Self {
        Self::start(Batches::new(Behavior::Silent))
    }

    /// The address of a server that refuses every connection.
    #[must_use]
    pub fn refusing() -> String {
        refusing_url()
    }

    fn start(batches: Batches) -> Self {
        let stored = Arc::clone(&batches.stored);
        let server = Server::start(batches);
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

fn named(versions: &[(&str, &str)]) -> Named {
    versions
        .iter()
        .map(|(key, code)| ((*key).to_owned(), (*code).to_owned()))
        .collect()
}

struct Batches {
    behavior: Behavior,
    refused: Named,
    put_off: Named,
    stored: Arc<Mutex<Vec<Value>>>,
}

impl Responder for Batches {
    fn answer(&mut self, request: &Request) -> Answer {
        match &self.behavior {
            Behavior::Silent => return Answer::Hold,
            Behavior::Failing(status) => return failure(*status).into(),
            Behavior::ComeBack {
                status,
                retry_after,
            } => {
                let mut response = failure(*status);
                response
                    .headers
                    .push(("Retry-After".to_owned(), retry_after.clone()));
                return response.into();
            }
            _ => {}
        }
        let records = match admit(request) {
            Ok(records) => records,
            Err(refusal) => return refusal.into(),
        };
        if let Behavior::Answering(body) = &self.behavior {
            let mut response = Response::json(200, &Value::Null);
            response.body = body.clone().into_bytes();
            return response.into();
        }
        let outcomes: Vec<Value> = records
            .into_iter()
            .map(|record| self.settle(record))
            .collect();
        let response = Response::json(
            200,
            &json!({ "format": ANSWER_FORMAT, "outcomes": outcomes }),
        );
        match self.behavior {
            Behavior::HangingUp => Answer::HangUp,
            Behavior::Late(after) => Answer::Late { response, after },
            _ => response.into(),
        }
    }
}

impl Batches {
    fn new(behavior: Behavior) -> Self {
        Self {
            behavior,
            refused: Vec::new(),
            put_off: Vec::new(),
            stored: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// The outcome of `record`'s version, storing the record where the version is new.
    fn settle(&self, record: Value) -> Value {
        let key = version_key(&record).unwrap_or_default().to_owned();
        let named = |list: &Named| {
            list.iter()
                .find(|(named, _)| *named == key)
                .map(|(_, code)| code.clone())
        };
        if let Some(code) = named(&self.refused) {
            return outcome(&key, "refused", Some(&code));
        }
        if let Some(code) = named(&self.put_off) {
            return outcome(&key, "later", Some(&code));
        }
        if !is_version(&record) {
            return outcome(&key, "refused", Some("invalid_record"));
        }
        let mut stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        match stored.iter().find(|held| version_key(held) == Some(&key)) {
            Some(held) if held["version"] == record["version"] => outcome(&key, "held", None),
            Some(_) => outcome(&key, "conflict", None),
            None => {
                stored.push(record);
                outcome(&key, "stored", None)
            }
        }
    }
}

/// One version's outcome, with its code where it has one.
fn outcome(key: &str, outcome: &str, code: Option<&str>) -> Value {
    let mut value = json!({ "version": key, "outcome": outcome });
    if let Some(code) = code {
        value["code"] = json!(code);
    }
    value
}

/// The records of `request`, or the refusal it is answered with.
fn admit(request: &Request) -> Result<Vec<Value>, Response> {
    if request.path != BATCHES {
        return Err(refusal("not_found"));
    }
    if request.method != "POST" {
        return Err(refusal("method_not_allowed"));
    }
    let bearer = format!("Bearer {TOKEN}");
    if request.header("Authorization") != Some(bearer.as_str()) {
        return Err(refusal("unauthenticated"));
    }
    let json = request
        .header("Content-Type")
        .and_then(|value| value.split(';').next())
        .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"));
    if !json {
        return Err(problem(415, "unsupported_media_type", Map::new()));
    }
    if request.body.len() > MOST_BYTES {
        return Err(refusal("payload_too_large"));
    }
    records(&request.body).ok_or_else(|| refusal("invalid_request"))
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

/// Whether `record` is a record of a batch of versions: a version, of a Yard Log, as an object.
fn is_version(record: &Value) -> bool {
    record.get("record").and_then(Value::as_str) == Some("version")
        && record.get("yardLog").and_then(Value::as_str).is_some()
        && record.get("version").is_some_and(Value::is_object)
}

/// The key of a record's version.
fn version_key(record: &Value) -> Option<&str> {
    record.get("version")?.get("key")?.as_str()
}

/// The problem document of an error scenario at `status`: the code the remote's closed set gives
/// that status, else `internal_error` for a server error and `invalid_request` for another.
fn failure(status: u16) -> Response {
    let code = CODES.iter().find(|(_, known)| *known == status).map_or(
        if status >= 500 {
            "internal_error"
        } else {
            "invalid_request"
        },
        |(code, _)| code,
    );
    problem(status, code, Map::new())
}
