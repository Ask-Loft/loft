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
pub mod wire;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};

use crate::http::{Request, Responder, Response, Server, refusing_url};

pub use oauth::Grant;

/// The bearer token every scenario admits, as `LOFT_REMOTE_TOKEN` gives it to the binary.
pub const TOKEN: &str = "simulator-token";

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

/// How the person answers an authorization request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consent {
    Grant,
    Deny,
}

#[derive(Clone, Debug)]
enum Mode {
    Accepting,
    Refusing,
    Failing { status: u16, code: Option<String> },
    Silent,
}

/// The document a scenario holds: its name, its published states and its drafts.
#[derive(Clone, Debug, Default)]
struct Held {
    name: String,
    states: Vec<StoredState>,
    drafts: BTreeMap<String, BTreeMap<String, String>>,
}

/// What a remote simulator holds and how it answers.
#[derive(Clone, Debug)]
pub struct RemoteScenario {
    key: String,
    mode: Mode,
    retry_after: Option<String>,
    held: Option<Held>,
    /// The calls that every token is forbidden: `create`, `import` or both.
    forbidden: Vec<&'static str>,
    closed: Option<String>,
    import_limit: Option<usize>,
    consent: Consent,
    expires_in: u64,
    grace: bool,
    lapsed: bool,
    scope: String,
    publish_from: u64,
}

impl RemoteScenario {
    /// A remote that answers as the contract states, holding no document under `key`.
    #[must_use]
    pub fn accepting(key: &str) -> Self {
        Self {
            key: key.to_owned(),
            mode: Mode::Accepting,
            retry_after: None,
            held: None,
            forbidden: Vec::new(),
            closed: None,
            import_limit: None,
            consent: Consent::Grant,
            expires_in: 86_400,
            grace: true,
            lapsed: false,
            scope: "documents".to_owned(),
            publish_from: 1_800_000_000,
        }
    }

    /// A remote that answers every request with `status`, and with a problem document of `code`
    /// where it names one.
    #[must_use]
    pub fn failing(key: &str, status: u16, code: Option<&str>) -> Self {
        Self {
            mode: Mode::Failing {
                status,
                code: code.map(str::to_owned),
            },
            ..Self::accepting(key)
        }
    }

    /// A remote that accepts each connection and never answers.
    #[must_use]
    pub fn silent(key: &str) -> Self {
        Self {
            mode: Mode::Silent,
            ..Self::accepting(key)
        }
    }

    /// A remote whose port refuses every connection.
    #[must_use]
    pub fn refusing(key: &str) -> Self {
        Self {
            mode: Mode::Refusing,
            ..Self::accepting(key)
        }
    }

    /// Holds the document under the scenario's key, named `name`, with `states` published.
    #[must_use]
    pub fn holding(mut self, name: &str, states: Vec<StoredState>) -> Self {
        let held = self.held.get_or_insert_with(Held::default);
        name.clone_into(&mut held.name);
        held.states = states;
        self
    }

    /// Holds a draft under `branch`, of `files` by their paths from the top of the tree.
    #[must_use]
    pub fn draft(mut self, branch: &str, files: BTreeMap<String, String>) -> Self {
        let held = self.held.get_or_insert_with(Held::default);
        held.drafts.insert(branch.to_owned(), files);
        self
    }

    /// Sends `Retry-After: value` with each failing answer.
    #[must_use]
    pub fn retry_after(mut self, value: &str) -> Self {
        self.retry_after = Some(value.to_owned());
        self
    }

    /// Refuses every create as `forbidden`.
    #[must_use]
    pub fn forbidding_create(mut self) -> Self {
        self.forbidden.push("create");
        self
    }

    /// Refuses every import as `forbidden`.
    #[must_use]
    pub fn forbidding_import(mut self) -> Self {
        self.forbidden.push("import");
        self
    }

    /// Refuses every publish as a `conflict` at the head, with `detail` as its reason.
    #[must_use]
    pub fn closed(mut self, detail: &str) -> Self {
        self.closed = Some(detail.to_owned());
        self
    }

    /// Refuses an import whose body is longer than `bytes` as `payload_too_large`.
    #[must_use]
    pub fn import_limit(mut self, bytes: usize) -> Self {
        self.import_limit = Some(bytes);
        self
    }

    /// The person refuses each authorization request.
    #[must_use]
    pub fn refusing_consent(mut self) -> Self {
        self.consent = Consent::Deny;
        self
    }

    /// Issues access tokens that expire in `seconds`.
    #[must_use]
    pub fn expiring_in(mut self, seconds: u64) -> Self {
        self.expires_in = seconds;
        self
    }

    /// Refuses a rotated refresh token presented again, and revokes its sign-in.
    #[must_use]
    pub fn without_grace(mut self) -> Self {
        self.grace = false;
        self
    }

    /// Refuses every refresh, as a sign-in that has lapsed.
    #[must_use]
    pub fn lapsing(mut self) -> Self {
        self.lapsed = true;
        self
    }

    /// Dates the remote's publishes from `seconds` after the Unix epoch, a second apart.
    #[must_use]
    pub fn publishing_from(mut self, seconds: u64) -> Self {
        self.publish_from = seconds;
        self
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
    pub fn start(scenario: RemoteScenario) -> Self {
        let key = scenario.key.clone();
        let state = Arc::new(Mutex::new(State {
            held: scenario.held.clone(),
            scenario,
            origin: String::new(),
            replies: HashMap::new(),
            auth: oauth::Auth::default(),
        }));
        let refusing = matches!(lock(&state).scenario.mode, Mode::Refusing);
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
    let resource_metadata = format!("/.well-known/oauth-protected-resource{document}");
    match path {
        _ if path == resource_metadata => only_get(request, || oauth::resource_metadata(state)),
        "/.well-known/oauth-authorization-server/auth" => {
            only_get(request, || oauth::server_metadata(state))
        }
        "/auth/register" => only_post(request, || oauth::register(state, request)),
        "/auth/authorize" => only_get(request, || oauth::authorize(state, query)),
        "/auth/token" => only_post(request, || oauth::token(state, request)),
        _ => problem::refusal("not_found"),
    }
}

fn only_get(request: &Request, answer: impl FnOnce() -> Response) -> Response {
    if request.method == "GET" {
        answer()
    } else {
        problem::refusal("method_not_allowed")
    }
}

fn only_post(request: &Request, answer: impl FnOnce() -> Response) -> Response {
    if request.method == "POST" {
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
