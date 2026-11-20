//! The remote simulator: an HTTP endpoint on the loopback interface that stands in for a
//! connected document's remote and for the authorization server that admits to it. A test
//! points the binary under test at it with `LOFT_REMOTE_ENDPOINT` and `LOFT_REMOTE_TOKEN`, or
//! signs in to it as a person would, scripts what it holds and how it answers, and reads the
//! requests it received.
//!
//! The document lies at `<origin>/documents/<key>`, the address the binary is given. The
//! authorization server's issuer is `<origin>/auth`, and both metadata documents lie under
//! `<origin>/.well-known/`, beside the metadata of each companion resource a scenario lists.
//! What a scenario places on another origin lies on a second port of the loopback interface, and
//! there alone; a scenario that names its protected resource on another origin names it there. A
//! publish is published with the Format's own publisher, made as the document's remote, so its
//! labels and its lock are the ones a remote would return.

mod companion;
mod documents;
mod oauth;
pub mod problem;
mod scenario;
mod skill;
pub mod wire;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

use crate::http::{Answer, Request, Responder, Response, Server, refusing_url};

pub use companion::{Companion, CompanionFault};
pub use documents::misstated_root;
pub use oauth::Grant;
use scenario::{Call, Held, Misstating, Mode};
pub use scenario::{Consent, Metadata, RemoteScenario};

/// The bearer token every scenario admits, as `LOFT_REMOTE_TOKEN` gives it to the binary.
pub const TOKEN: &str = "simulator-token";

/// The path of the protected resource every document lies under, which the resource metadata
/// names and is served beside.
const RESOURCE: &str = "/documents";

/// Where a protected resource metadata document lies on its resource's origin, before the
/// resource's path and query (RFC 9728).
const WELL_KNOWN: &str = "/.well-known/oauth-protected-resource";

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
    /// The origin of the second port, which stands for another origin, if the scenario places
    /// metadata, or names its protected resource, there.
    elsewhere: Option<String>,
    held: Option<Held>,
    replies: HashMap<String, Value>,
    auth: oauth::Auth,
    /// Whether the answer to the request being answered is lost.
    answer_lost: bool,
}

impl State {
    fn address_path(&self) -> String {
        format!("/documents/{}", self.scenario.key)
    }
}

/// A running remote simulator.
pub struct Remote {
    server: Option<Server>,
    /// The second port, which stands for another origin, if the scenario places metadata, or
    /// names its protected resource, there.
    elsewhere: Option<Server>,
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
        let placed_elsewhere = scenario.places_elsewhere();
        let state = Arc::new(Mutex::new(State {
            held: scenario.held.take(),
            scenario,
            origin: String::new(),
            elsewhere: None,
            replies: HashMap::new(),
            auth: oauth::Auth::default(),
            answer_lost: false,
        }));
        let (server, origin) = listen(&state, refusing, route);
        let (elsewhere, elsewhere_origin) = if placed_elsewhere {
            let (server, origin) = listen(&state, refusing, companion::elsewhere);
            (server, Some(origin))
        } else {
            (None, None)
        };
        let mut locked = lock(&state);
        locked.origin.clone_from(&origin);
        locked.elsewhere = elsewhere_origin;
        drop(locked);
        Self {
            server,
            elsewhere,
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

    /// The document's protected resource, as `<origin>/documents`.
    #[must_use]
    pub fn resource(&self) -> String {
        lock(&self.state).resource()
    }

    /// The issuer of the simulator's authorization server, as `<origin>/auth`.
    #[must_use]
    pub fn issuer(&self) -> String {
        lock(&self.state).issuer()
    }

    /// The identifier that the document's protected resource metadata lists for the companion
    /// resource of `kind`.
    ///
    /// # Panics
    ///
    /// The scenario lists no companion resource of `kind`.
    #[must_use]
    pub fn companion(&self, kind: &str) -> String {
        lock(&self.state)
            .companion_identifier(kind)
            .unwrap_or_else(|| {
                panic!("the scenario lists no companion resource of the kind {kind}")
            })
    }

    /// Every request received so far, in the order received.
    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        self.server
            .as_ref()
            .map(Server::requests)
            .unwrap_or_default()
    }

    /// Every request that the second port received so far, in the order received: the port of
    /// the loopback interface that stands for another origin, which a scenario starts when it
    /// places metadata, or names its protected resource, there.
    #[must_use]
    pub fn requests_elsewhere(&self) -> Vec<Request> {
        self.elsewhere
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
    /// client, an access token for the document's protected resource and a refresh token.
    #[must_use]
    pub fn grant(&self) -> Grant {
        self.grant_over(&[self.resource().as_str()])
    }

    /// A sign-in over `resources` issued without a browser: a registered client, an access token
    /// for each resource alone, the first of them as `access_token`, and one refresh token. The
    /// sign-in is granted every scope that a resource it holds accepts. A resource the scenario
    /// does not serve, such as one it served before, accepts no scope, and its access token is
    /// admitted by that resource alone.
    ///
    /// # Panics
    ///
    /// `resources` is empty, or names a resource more than once: a sign-in holds at least one
    /// resource, and each once, as no authorization request that names a resource twice is
    /// answered with a code.
    #[must_use]
    pub fn grant_over(&self, resources: &[&str]) -> Grant {
        assert!(
            !resources.is_empty(),
            "a sign-in holds at least one resource, and none was named"
        );
        let repeated = (1..resources.len()).find(|at| resources[..*at].contains(&resources[*at]));
        if let Some(at) = repeated {
            panic!(
                "a sign-in holds each resource once, and {} was named more than once",
                resources[at]
            );
        }
        let mut state = lock(&self.state);
        let expires_in = state.scenario.expires_in;
        let accepted: Vec<(String, Vec<String>)> = resources
            .iter()
            .map(|resource| {
                let scopes = state.accepted(resource).unwrap_or_default();
                ((*resource).to_owned(), scopes)
            })
            .collect();
        state.auth.grant(expires_in, &accepted)
    }

    /// Moves the simulator's clock on by `by`, without waiting.
    pub fn advance(&self, by: Duration) {
        lock(&self.state).auth.moved += by;
    }

    /// Whether a request under the document's address bearing `token` is admitted now.
    #[must_use]
    pub fn admits(&self, token: &str) -> bool {
        lock(&self.state).admits(token)
    }

    /// Whether the protected resource `resource` admits the access token `token` now: a token
    /// the simulator issued for that resource, not yet expired, of a sign-in it has not revoked.
    /// A companion resource has no routes of its own, so a test asks this in place of a request.
    #[must_use]
    pub fn admits_at(&self, resource: &str, token: &str) -> bool {
        lock(&self.state).auth.admits_at(resource, token)
    }
}

fn lock(state: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The answer of an accepting simulator to a request on one of its ports.
type Route = fn(&mut State, &Request) -> Response;

/// A server on a free port of the loopback interface that answers from `state` by `route`, and
/// its origin; or, for a refusing scenario, no server and a port where nothing listens.
fn listen(state: &Arc<Mutex<State>>, refusing: bool, route: Route) -> (Option<Server>, String) {
    if refusing {
        return (None, refusing_url());
    }
    let server = Server::start(Simulated {
        state: Arc::clone(state),
        route,
    });
    let origin = server.url();
    (Some(server), origin)
}

/// A port of the simulator: its state, shared with the test and its other port, and how it
/// answers a scenario that accepts.
struct Simulated {
    state: Arc<Mutex<State>>,
    route: Route,
}

impl Responder for Simulated {
    fn answer(&mut self, request: &Request) -> Answer {
        let mut state = lock(&self.state);
        let mut response = match &state.scenario.mode {
            Mode::Silent => return Answer::Hold,
            Mode::Failing { status, code } => match code {
                Some(code) => problem::problem(*status, code, serde_json::Map::new()),
                None => problem::plain(*status),
            },
            Mode::Accepting | Mode::Refusing => (self.route)(&mut state, request),
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
        // A refresh is delayed, or its answer lost, with the state released, so that neither
        // the test nor the simulator's next request waits on it.
        let delay = oauth::refreshing(request).then_some(state.scenario.refresh_delay);
        let lost = std::mem::take(&mut state.answer_lost);
        drop(state);
        thread::sleep(delay.unwrap_or_default());
        if lost {
            Answer::HangUp
        } else {
            response.into()
        }
    }
}

/// The answer of an accepting simulator on its origin, by the request's path: a companion
/// resource's metadata by the whole target, its path and its query, before the document's
/// protected resource metadata, whose path a companion resource's may share.
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
    if let Some(answer) = state.companion_metadata(request) {
        return answer;
    }
    let resource_metadata = format!("{WELL_KNOWN}{RESOURCE}");
    // A scenario that places the document's protected resource metadata on another origin
    // serves it there alone; one that names its protected resource there serves it here at the
    // address its challenge names, besides its usual one.
    let metadata_here = state.scenario.metadata != Metadata::OnAnotherOrigin;
    let naming_elsewhere = state.path_naming_elsewhere();
    match path {
        _ if metadata_here && path == resource_metadata => {
            only("GET", request, || oauth::resource_metadata(state))
        }
        _ if naming_elsewhere.as_deref() == Some(path) => only("GET", request, || {
            oauth::metadata_naming_elsewhere(state, path)
        }),
        "/.well-known/oauth-authorization-server/auth" => {
            only("GET", request, || oauth::server_metadata(state))
        }
        "/auth/register" => only("POST", request, || oauth::register(state, request)),
        "/auth/authorize" => only("GET", request, || oauth::authorize(state, query)),
        "/auth/token" => only("POST", request, || oauth::token(state, request)),
        skill::MANIFEST => only("GET", request, || skill::manifest(state)),
        _ => match skill::package(state, path) {
            Some(answer) => only("GET", request, || answer),
            None => problem::refusal("not_found"),
        },
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

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::{Remote, RemoteScenario};

    /// The message with which `remote` refuses a sign-in over `resources` granted without a
    /// browser.
    fn refusal(remote: &Remote, resources: &[&str]) -> String {
        let refused = catch_unwind(AssertUnwindSafe(|| remote.grant_over(resources)));
        let payload = refused.expect_err("the sign-in is refused");
        let message = payload
            .downcast_ref::<&str>()
            .map(|message| (*message).to_owned());
        message
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_default()
    }

    #[test]
    fn a_sign_in_granted_without_a_browser_holds_at_least_one_resource_and_each_once() {
        let remote = Remote::start(RemoteScenario::accepting("key"));
        let none = refusal(&remote, &[]);
        assert!(none.contains("at least one resource"), "{none:?}");
        let own = remote.resource();
        let former = format!("{}/former", remote.origin());
        for (named, twice) in [
            (vec![own.as_str(), own.as_str()], own.as_str()),
            (
                vec![own.as_str(), former.as_str(), former.as_str()],
                former.as_str(),
            ),
        ] {
            let refused = refusal(&remote, &named);
            let says = refused.contains("each resource once") && refused.contains(twice);
            assert!(says, "{named:?}: {refused:?}");
        }
        let grant = remote.grant_over(&[&own, &former]);
        assert!(remote.admits_at(&own, &grant.access_token));
        assert!(remote.admits_at(&former, &grant.access[&former]));
    }
}
