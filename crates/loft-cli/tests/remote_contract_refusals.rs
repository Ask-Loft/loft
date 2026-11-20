//! The contract suite of the authorization requests that a remote refuses before any person
//! consents, answered as RFC 6749, section 4.1.2.1, says. A request from a client that is not
//! registered, or to a redirect URI that matches none registered, is answered `400 Bad Request`
//! without a redirect. Any other is redirected back to the client with `error`: for a PKCE
//! challenge other than `S256`; a `response_type` left out or other than `code`; a resource left
//! out, not served or named twice; a `scope` that is empty, repeats a scope or holds one that no
//! resource it names lists; or a resource asked for none of the scopes it lists. A request that
//! fails several steps is answered for the first, in the order the remote judges them.
//!
//! Each scenario runs against the simulator and, on demand, against the remote at
//! `LOFT_REMOTE_CONTRACT_ADDRESS`: run a real half with
//! `cargo nextest run -p loft-cli --run-ignored only -E 'test(/<its name>/)'`. A real half
//! registers a client of its own with the remote's authorization server and needs no sign-in, so
//! the real halves may run together. It reads the remote's own protected resource, the scopes it
//! lists and its companion resources from the remote's metadata, and shares its scenario with its
//! simulator half, so that the two cannot drift. A real half that needs a companion resource says
//! so, and fails naming its premise where the remote lists none that it serves and its scenario
//! can ask, so that no step passes for a refusal of a companion resource the remote does not
//! serve; none passes without checking. A real half that gets any answer but the refusal it
//! expects fails, naming the request and showing the answer's status, `Location` and body with
//! every token and code hidden, so that a remote that answers a refusal otherwise, or judges the
//! steps in another order, is told plainly. The simulator alone is asked what needs a person: a
//! code, and a consent it refuses, which shows that each refusal comes before the person's
//! consent.

use std::any::Any;
use std::collections::BTreeMap;
use std::env;
use std::panic::{AssertUnwindSafe, catch_unwind};

use serde_json::{Value, json};
use software_plant::remote::wire::value;
use software_plant::remote::{Companion, CompanionFault, Remote, RemoteScenario};

use crate::common::calls::Answer;
use crate::common::oauth::{
    CALLBACK, KEY, UNSERVED, authorized, coded_for, companion_metadata, discover, exchange,
    location, registered,
};
use crate::common::resources::{companions_of, origin_of};

/// A client id that no remote registered.
const UNREGISTERED: &str = "loft-contract-unregistered-client";
/// A redirect URI on the loopback interface whose path matches none that a client registers.
const UNMATCHED: &str = "http://127.0.0.1:4567/elsewhere";
/// A scope that no remote lists.
const UNKNOWN_SCOPE: &str = "loft-contract-unknown-scope";
/// The premise of each real half that asks a companion resource.
const APART: &str = "premise: the remote lists a companion resource that it serves, on the origin \
                     of its own protected resource, whose metadata names it and the remote's \
                     issuer and lists a scope, and none of the scopes that the remote's own \
                     protected resource lists";

/// A client registered with the authorization server of a document's remote, and what it read of
/// the remote's protected resource metadata.
struct Client {
    /// The authorization server's metadata.
    server: Value,
    /// The client's id.
    id: String,
    /// The remote's own protected resource.
    own: String,
    /// The scopes that the remote's own protected resource lists.
    scopes: Vec<String>,
    /// The companion resources that the remote lists.
    companions: Vec<String>,
}

/// A client registered with the authorization server of the remote of the document at `address`,
/// which found the remote's metadata from the challenge of a request without a token.
fn client_of(address: &str) -> Client {
    let (metadata, server) = discover(address);
    let own = metadata["resource"]
        .as_str()
        .expect("protected resource metadata that names its resource");
    let scopes = listed(&metadata);
    assert!(!scopes.is_empty(), "a scope of {own}: {metadata}");
    let companions = companions_of(&metadata).into_iter().map(|(at, _)| at);
    Client {
        id: registered(&server),
        server,
        own: own.to_owned(),
        scopes,
        companions: companions.collect(),
    }
}

/// A client registered with the authorization server of the remote of the document at
/// `LOFT_REMOTE_CONTRACT_ADDRESS`.
fn real_client() -> Client {
    client_of(&env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address"))
}

/// The scopes that `metadata`, a resource's protected resource metadata, lists, in its order.
fn listed(metadata: &Value) -> Vec<String> {
    let scopes = metadata["scopes_supported"]
        .as_array()
        .into_iter()
        .flatten();
    scopes
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

impl Client {
    /// The first scope that the remote's own protected resource lists.
    fn scope(&self) -> &str {
        &self.scopes[0]
    }

    /// The answer to the client's authorization request for `resources` and `scope`, with
    /// `overrides` replacing its parameters.
    fn asks(&self, resources: &[&str], scope: &str, overrides: &[(&str, &str)]) -> Answer {
        authorized(&self.server, &self.id, resources, scope, overrides)
    }

    /// Requires the client's authorization request for `resources` and `scope`, with `overrides`
    /// replacing its parameters, to redirect back to the client with `error`.
    fn refused(&self, resources: &[&str], scope: &str, overrides: &[(&str, &str)], error: &str) {
        let answer = self.asks(resources, scope, overrides);
        let asked = format!("{resources:?} asking {scope:?} with {overrides:?}");
        assert_redirected(&answer, &asked, error);
    }

    /// A companion resource that the remote lists and serves, none of whose scopes the remote's
    /// own protected resource lists, with the first scope it lists. The remote serves it where it
    /// lies on the origin of the remote's own protected resource and its metadata names it and the
    /// remote's issuer, so that a refusal of a request for it cannot come of its being unserved.
    fn apart(&self) -> (&str, String) {
        let issuer = &self.server["issuer"];
        let found = self.companions.iter().find_map(|companion| {
            let metadata = companion_metadata(companion).body;
            let named = metadata["resource"] == json!(companion);
            let servers = metadata["authorization_servers"].as_array();
            let vouched = servers.is_some_and(|servers| servers.contains(issuer));
            let here = origin_of(companion) == origin_of(&self.own);
            let theirs = listed(&metadata);
            let shared = theirs.iter().any(|scope| self.scopes.contains(scope));
            let served = named && vouched && here;
            let first = theirs.into_iter().next().filter(|_| served && !shared)?;
            Some((companion.as_str(), first))
        });
        let companions = &self.companions;
        found
            .unwrap_or_else(|| panic!("{APART} (the companion resources it lists: {companions:?})"))
    }
}

/// Requires `answer`, to the authorization request `asked`, to redirect back to the client with
/// `error`. A failure names the request and the error, and shows the answer with every token and
/// code hidden.
fn assert_redirected(answer: &Answer, asked: &str, error: &str) {
    let expected = format!("{asked}: a redirect back to the client with `error` `{error}`");
    let back = answer
        .header("location")
        .is_some_and(|at| at.starts_with(&format!("{CALLBACK}?")));
    assert!(answer.status == 302 && back, "{expected}: {answer:?}");
    let answered = value(&location(answer), "error").map(str::to_owned);
    assert_eq!(answered.as_deref(), Some(error), "{expected}: {answer:?}");
}

/// Requires `answer`, to the authorization request `asked`, to be `400 Bad Request` without a
/// redirect. A failure shows the answer with every token and code hidden.
fn assert_unredirected(answer: &Answer, asked: &str) {
    let unredirected = answer.status == 400 && answer.header("location").is_none();
    let expected = "`400 Bad Request` without a redirect";
    assert!(unredirected, "{asked}: {expected}: {answer:?}");
}

/// Refuses each authorization request of `client` without a PKCE challenge of method `S256`
/// `invalid_request`, by a redirect back to the client at [`CALLBACK`], a loopback redirect URI
/// on a port that the client did not register; and answers one from a client that is not
/// registered, or to a redirect URI whose path matches none registered, `400 Bad Request` without
/// a redirect.
fn refuses_without_pkce_or_from_an_unregistered_client_or_redirect(client: &Client) {
    let (own, scope) = (client.own.as_str(), client.scope());
    // A parameter sent without a value is read as one left out (RFC 6749, section 3.1), and a
    // challenge without a method is a `plain` one (RFC 7636, section 4.3).
    for unchallenged in [
        ("code_challenge_method", "plain"),
        ("code_challenge_method", ""),
        ("code_challenge", ""),
    ] {
        client.refused(&[own], scope, &[unchallenged], "invalid_request");
    }
    for (id, redirect) in [(UNREGISTERED, CALLBACK), (client.id.as_str(), UNMATCHED)] {
        let answer = client.asks(
            &[own],
            scope,
            &[("client_id", id), ("redirect_uri", redirect)],
        );
        assert_unredirected(&answer, &format!("{id} to {redirect}"));
    }
}

#[test]
fn an_authorization_request_without_pkce_or_from_an_unregistered_client_or_redirect_is_refused() {
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    refuses_without_pkce_or_from_an_unregistered_client_or_redirect(&client_of(&remote.address()));
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_refuses_an_authorization_request_without_pkce_or_from_an_unregistered_client_or_redirect()
 {
    refuses_without_pkce_or_from_an_unregistered_client_or_redirect(&real_client());
}

/// Refuses each authorization request of `client` that names no resource, a resource that the
/// remote does not serve, or one resource twice, `invalid_target`; one whose `scope` is empty,
/// repeats a scope or holds one that no resource it names lists, `invalid_scope`; and one whose
/// `response_type` is left out `invalid_request`, or other than `code`
/// `unsupported_response_type`.
fn refuses_a_resource_or_a_scope_it_does_not_serve(client: &Client) {
    let (own, scope) = (client.own.as_str(), client.scope());
    let (twice, unknown) = (
        format!("{scope} {scope}"),
        format!("{scope} {UNKNOWN_SCOPE}"),
    );
    for (resources, asked, refused) in [
        (vec![], scope, "invalid_target"),
        (vec![own, UNSERVED], scope, "invalid_target"),
        (vec![own, own], scope, "invalid_target"),
        (vec![own], "", "invalid_scope"),
        (vec![own], twice.as_str(), "invalid_scope"),
        (vec![own], unknown.as_str(), "invalid_scope"),
    ] {
        client.refused(&resources, asked, &[], refused);
    }
    // A parameter sent without a value is read as one left out (RFC 6749, section 3.1).
    for (response_type, refused) in [
        ("", "invalid_request"),
        ("token", "unsupported_response_type"),
    ] {
        client.refused(&[own], scope, &[("response_type", response_type)], refused);
    }
}

/// Refuses each authorization request of `client` that asks a companion resource amiss: one that
/// names the remote's own resource alone and asks a scope that only the companion resource lists,
/// `invalid_scope`; one that names both and asks the companion resource for none of its scopes,
/// `invalid_target`; and one that names the companion resource twice, `invalid_target`.
fn refuses_a_companion_resource_asked_amiss(client: &Client) {
    let (own, scope) = (client.own.as_str(), client.scope());
    let (companion, theirs) = client.apart();
    let both = format!("{scope} {theirs}");
    for (resources, asked, refused) in [
        (vec![own], both.as_str(), "invalid_scope"),
        (vec![own, companion], scope, "invalid_target"),
        (
            vec![own, companion, companion],
            both.as_str(),
            "invalid_target",
        ),
    ] {
        client.refused(&resources, asked, &[], refused);
    }
}

#[test]
fn an_authorization_request_is_refused_for_a_resource_or_a_scope_the_remote_does_not_serve() {
    let remote = Remote::start(
        RemoteScenario::accepting(KEY).accompanied_by(Companion::new("notes", &["notes"])),
    );
    let client = client_of(&remote.address());
    refuses_a_resource_or_a_scope_it_does_not_serve(&client);
    refuses_a_companion_resource_asked_amiss(&client);
    // A remote that lists no companion resource refuses the same requests. Whether or not it
    // lists any, a request for the document's protected resource alone gets a code for it.
    let unaccompanied = Remote::start(RemoteScenario::accepting(KEY));
    refuses_a_resource_or_a_scope_it_does_not_serve(&client_of(&unaccompanied.address()));
    for remote in [&remote, &unaccompanied] {
        let own = remote.resource();
        let (server, client, code) = coded_for(remote, &[&own], "documents");
        let answer = exchange(&server, &client, &own, &code);
        assert_eq!(answer.status, 200, "{answer:?}");
        assert_eq!(answer.body["resource"], json!(own));
    }
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_refuses_an_authorization_request_for_a_resource_or_a_scope_it_does_not_serve() {
    refuses_a_resource_or_a_scope_it_does_not_serve(&real_client());
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_refuses_an_authorization_request_that_asks_a_companion_resource_amiss() {
    // Needs a remote that lists a companion resource none of whose scopes its own protected
    // resource lists.
    refuses_a_companion_resource_asked_amiss(&real_client());
}

/// Answers each authorization request of `client` that fails a step and a later one for the
/// first, in the order the remote judges them: its client and redirect URI, its `response_type`,
/// its PKCE challenge, its resources, its `scope`, then whether it asks each resource it names
/// for a scope that the resource lists, which comes before any person consents.
fn answers_the_first_step_it_fails(client: &Client) {
    let (own, scope) = (client.own.as_str(), client.scope());
    // A client that is not registered, or a redirect URI that matches none registered, is
    // answered without a redirect, whatever else the request fails.
    for (id, redirect) in [(UNREGISTERED, CALLBACK), (client.id.as_str(), UNMATCHED)] {
        let failing = [
            ("client_id", id),
            ("redirect_uri", redirect),
            ("response_type", ""),
        ];
        let answer = client.asks(&[own], scope, &failing);
        assert_unredirected(&answer, &format!("{failing:?}"));
    }
    let plain = ("code_challenge_method", "plain");
    for (resources, asked, failing, error) in [
        (
            vec![own],
            scope,
            vec![("response_type", "token"), plain],
            "unsupported_response_type",
        ),
        (vec![UNSERVED], scope, vec![plain], "invalid_request"),
        (vec![UNSERVED], "", vec![], "invalid_target"),
        (vec![own, own], "", vec![], "invalid_target"),
        (vec![own], "", vec![], "invalid_scope"),
    ] {
        client.refused(&resources, asked, &failing, error);
    }
    // A request that asks a resource it names for none of the scopes it lists fails no earlier
    // step, and is refused before any person consents.
    let (companion, _) = client.apart();
    client.refused(&[own, companion], scope, &[], "invalid_target");
}

#[test]
fn an_authorization_request_is_answered_for_the_first_step_it_fails_in_the_order_judged() {
    // The scenario refuses consent, so that a request judged past every other step is answered
    // `access_denied`.
    let remote = Remote::start(
        RemoteScenario::accepting(KEY)
            .accompanied_by(Companion::new("notes", &["notes"]))
            .refusing_consent(),
    );
    let client = client_of(&remote.address());
    answers_the_first_step_it_fails(&client);
    let (own, notes) = (remote.resource(), remote.companion("notes"));
    let denied = client.asks(&[&own, &notes], "documents notes", &[]);
    let asked = "a request that fails no step before the person's consent";
    assert_redirected(&denied, asked, "access_denied");
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_answers_an_authorization_request_for_the_first_step_it_fails_in_the_order_judged()
{
    // Needs a remote that lists a companion resource none of whose scopes its own protected
    // resource lists, for the last step.
    answers_the_first_step_it_fails(&real_client());
}

/// The message of a failure whose payload is `payload`.
fn failure(payload: &(dyn Any + Send)) -> String {
    match (
        payload.downcast_ref::<String>(),
        payload.downcast_ref::<&str>(),
    ) {
        (Some(message), _) => message.clone(),
        (None, Some(message)) => (*message).to_owned(),
        (None, None) => String::new(),
    }
}

#[test]
fn a_redirect_where_none_is_expected_fails_without_showing_its_code() {
    let location = format!("{CALLBACK}?code=a-live-code&state=the-state");
    let redirected = Answer {
        status: 302,
        headers: BTreeMap::from([("location".to_owned(), location)]),
        body: Value::Null,
    };
    let failed = catch_unwind(AssertUnwindSafe(|| {
        assert_unredirected(&redirected, "a request");
    }));
    let payload = failed.expect_err("a redirect is not `400 Bad Request` without one");
    let message = failure(payload.as_ref());
    let hidden = message.contains("code=<redacted>") && !message.contains("a-live-code");
    assert!(hidden, "{message}");
}

#[test]
fn a_real_half_takes_as_its_premise_only_a_companion_resource_that_the_remote_serves() {
    let unserved = [
        Companion::new("misnamed", &["misnamed"]).faulty(CompanionFault::Misnamed),
        Companion::new("other", &["other"]).faulty(CompanionFault::UnderAnotherIssuer),
        Companion::new("away", &["away"]).elsewhere(),
    ];
    for companion in &unserved {
        let scenario = RemoteScenario::accepting(KEY).accompanied_by(companion.clone());
        let remote = Remote::start(scenario);
        let client = client_of(&remote.address());
        let failed = catch_unwind(AssertUnwindSafe(|| client.apart().0.to_owned()));
        let payload = failed.expect_err("an unserved companion resource is no premise");
        let message = failure(payload.as_ref());
        assert!(message.starts_with(APART), "{companion:?}: {message}");
    }
    // Listed after them, a companion resource that the remote serves is the one taken.
    let listed = unserved
        .into_iter()
        .fold(RemoteScenario::accepting(KEY), |scenario, companion| {
            scenario.accompanied_by(companion)
        });
    let remote = Remote::start(listed.accompanied_by(Companion::new("notes", &["notes"])));
    let client = client_of(&remote.address());
    let (companion, scope) = client.apart();
    assert_eq!(
        (companion, scope.as_str()),
        (remote.companion("notes").as_str(), "notes")
    );
}
