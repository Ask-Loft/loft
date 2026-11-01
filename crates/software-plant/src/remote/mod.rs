//! The remote simulator: an HTTP endpoint on the loopback interface that stands in for a
//! connected document's remote and for the authorization server that admits to it. A test
//! points the binary under test at it with `LOFT_REMOTE_ENDPOINT` and `LOFT_REMOTE_TOKEN`, or
//! signs in to it as a person would, scripts what it holds and how it answers, and reads the
//! requests it received.
//!
//! The document lies at `<origin>/documents/<key>`, the address the binary is given. The
//! authorization server's issuer is `<origin>/auth`, and both metadata documents lie under
//! `<origin>/.well-known/`. A publish is published with the Format's own publisher, made as the
//! document's remote, so its labels and its lock are the ones a remote would return.

mod documents;
mod oauth;
pub mod problem;
mod scenario;
pub mod wire;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};

use crate::http::{Request, Responder, Response, Server, refusing_url};

pub use oauth::Grant;
use scenario::misstated;
use scenario::{Call, Held, Misstating, Mode};
pub use scenario::{Consent, Metadata, RemoteScenario};

/// The bearer token every scenario admits, as `LOFT_REMOTE_TOKEN` gives it to the binary.
pub const TOKEN: &str = "simulator-token";

/// The path of the protected resource every document lies under, which the resource metadata
/// names and is served beside.
const RESOURCE: &str = "/documents";

/// A published state as the remote holds it: its lock record and the text of each file under
/// `spec/`, by its path from the top of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredState {
    pub lock: Value,
    pub files: BTreeMap<String, String>,
}

impl StoredState {
    /// The state's publish sequence.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.lock["publishSequence"].as_u64().unwrap_or(0)
    }
}

/// The simulator's state, shared between its server and the test.
struct State {
    scenario: RemoteScenario,
    origin: String,
    held: Option<Held>,
    replies: HashMap<String, Value>,
    auth: oauth::Auth,
}

impl State {
    fn address_path(&self) -> String {
        format!("/documents/{}", self.scenario.key)
    }
}

/// A running remote simulator.
pub struct Remote {
    server: Option<Server>,
    origin: String,
    key: String,
    state: Arc<Mutex<State>>,
}

impl Remote {
    /// Starts a remote simulator for `scenario` on a free port of the loopback interface, or,
    /// for a refusing scenario, names a port where nothing listens.
    #[must_use]
    pub fn start(mut scenario: RemoteScenario) -> Self {
        let key = scenario.key.clone();
        let refusing = matches!(scenario.mode, Mode::Refusing);
        let state = Arc::new(Mutex::new(State {
            held: scenario.held.take(),
            scenario,
            origin: String::new(),
            replies: HashMap::new(),
            auth: oauth::Auth::default(),
        }));
        let (server, origin) = if refusing {
            (None, refusing_url())
        } else {
            let server = Server::start(Simulated {
                state: Arc::clone(&state),
            });
            let origin = server.url();
            (Some(server), origin)
        };
        lock(&state).origin.clone_from(&origin);
        Self {
            server,
            origin,
            key,
            state,
        }
    }

    /// The origin of the simulator, as `http://127.0.0.1:<port>`.
    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The document's address, which names one origin and path.
    #[must_use]
    pub fn address(&self) -> String {
        format!("{}/documents/{}", self.origin, self.key)
    }

    /// Every request received so far, in the order received.
    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        self.server
            .as_ref()
            .map(Server::requests)
            .unwrap_or_default()
    }

    /// The published states the remote holds, from publish sequence 1.
    #[must_use]
    pub fn states(&self) -> Vec<StoredState> {
        lock(&self.state)
            .held
            .as_ref()
            .map(|held| held.states.clone())
            .unwrap_or_default()
    }

    /// The name of the document the remote holds, if it holds one.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        lock(&self.state)
            .held
            .as_ref()
            .map(|held| held.name.clone())
    }

    /// How many refreshes the token endpoint accepted.
    #[must_use]
    pub fn refreshes(&self) -> usize {
        lock(&self.state).auth.refreshes
    }

    /// A sign-in issued without a browser, as an accepted exchange would issue it: a registered
    /// client, an access token and a refresh token.
    #[must_use]
    pub fn grant(&self) -> Grant {
        let mut state = lock(&self.state);
        let expires_in = state.scenario.expires_in;
        state.auth.grant(expires_in)
    }

    /// Moves the simulator's clock on by `by`, without waiting.
    pub fn advance(&self, by: Duration) {
        lock(&self.state).auth.moved += by;
    }

    /// Whether the access token `token` is admitted now.
    #[must_use]
    pub fn admits(&self, token: &str) -> bool {
        lock(&self.state).admits(token)
    }
}

fn lock(state: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

struct Simulated {
    state: Arc<Mutex<State>>,
}

impl Responder for Simulated {
    fn answer(&mut self, request: &Request) -> Option<Response> {
        let mut state = lock(&self.state);
        let mut response = match &state.scenario.mode {
            Mode::Silent => return None,
            Mode::Failing { status, code } => match code {
                Some(code) => problem::problem(*status, code, serde_json::Map::new()),
                None => problem::plain(*status),
            },
            Mode::Accepting | Mode::Refusing => route(&mut state, request),
        };
        let retry = state
            .scenario
            .retry_after
            .clone()
            .or_else(|| (response.status == 429).then(|| "1".to_owned()));
        if let Some(retry) = retry
            && response.status >= 400
        {
            response.headers.push(("Retry-After".to_owned(), retry));
        }
        Some(response)
    }
}

/// The answer of an accepting simulator, by the request's path.
fn route(state: &mut State, request: &Request) -> Response {
    let (path, query) = request
        .path
        .split_once('?')
        .unwrap_or((request.path.as_str(), ""));
    let document = state.address_path();
    if let Some(rest) = path.strip_prefix(&document)
        && (rest.is_empty() || rest.starts_with('/'))
    {
        return documents::answer(state, request, rest);
    }
    let resource_metadata = format!("/.well-known/oauth-protected-resource{RESOURCE}");
    match path {
        _ if path == resource_metadata => only("GET", request, || oauth::resource_metadata(state)),
        "/.well-known/oauth-authorization-server/auth" => {
            only("GET", request, || oauth::server_metadata(state))
        }
        "/auth/register" => only("POST", request, || oauth::register(state, request)),
        "/auth/authorize" => only("GET", request, || oauth::authorize(state, query)),
        "/auth/token" => only("POST", request, || oauth::token(state, request)),
        _ => problem::refusal("not_found"),
    }
}

/// The answer of `answer` to a request by `method`, and `method_not_allowed` to any other.
fn only(method: &str, request: &Request, answer: impl FnOnce() -> Response) -> Response {
    if request.method == method {
        answer()
    } else {
        problem::refusal("method_not_allowed")
    }
}

/// The head as a `conflict` refusal names it: its publish sequence and state root, `0` and
/// `null` for a document with no published state.
fn head_mark(held: &Held) -> Value {
    held.states.last().map_or_else(
        || json!({ "publishSequence": 0, "stateRoot": null }),
        |state| {
            json!({
                "publishSequence": state.sequence(),
                "stateRoot": state.lock["stateRoot"],
            })
        },
    )
}
