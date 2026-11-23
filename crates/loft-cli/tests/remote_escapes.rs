//! Text that a remote, its authorization server or `loft/remotes.json` gave, named in an error of
//! reaching the remote or of signing in to it: each control character and each bidirectional
//! control is written escaped, on standard error and in the JSON document's `message` alike; and
//! a resource or an issuer that a sign-in's metadata names outside visible ASCII is refused, as a
//! remote's address is. Each runs under the remote and browser simulators, or under a remote of
//! the test's own where no scenario of the simulator gives such text.

use std::process::Output;

use serde_json::json;
use software_plant::BrowserScenario;
use software_plant::http::{Answer, Request, Responder, Response, Server};
use software_plant::remote::wire::{pairs, query, value};
use software_plant::remote::{Remote, RemoteScenario};

use crate::common::connected::connected;
use crate::common::signed_in::{document, granted, keep, name_remote, run, sign_ins_file};
use crate::common::{Scratch, stderr, tree};
use crate::remote_address_characters::RULE;

/// Text that holds control characters of both ranges, a bidirectional control and a backslash.
pub const HOSTILE: &str = "x\u{1b}]0;y\u{7}\u{9b}2J\u{202e}\\z";
/// [`HOSTILE`] as an error writes it.
pub const ESCAPED: &str = "x\\u{1b}]0;y\\u{7}\\u{9b}2J\\u{202e}\\\\z";

/// Text that a header can carry and an address may not: a tab, the one control character a
/// header's value holds.
const TABBED: &str = "a\tb";
/// [`TABBED`] as an error writes it.
const TABBED_ESCAPED: &str = "a\\u{9}b";

/// Whether `text`, one line, holds no control character, of Unicode's general category `Cc`, and
/// no bidirectional control, of Unicode's property `Bidi_Control`.
pub fn visible(text: &str) -> bool {
    !text.chars().any(|character| {
        character.is_control()
            || matches!(character, '\u{61c}' | '\u{200e}' | '\u{200f}')
            || ('\u{202a}'..='\u{202e}').contains(&character)
            || ('\u{2066}'..='\u{2069}').contains(&character)
    })
}

/// The `message` of the first error of the JSON document a run printed.
pub fn message(output: &Output) -> String {
    document(output)["errors"][0]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// Asserts that a run that failed named `named` on standard error and in its JSON document's
/// `message`, the message as standard error shows it, and that neither holds a control character
/// or a bidirectional control.
pub fn assert_named(output: &Output, named: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let (stderr, message) = (stderr(output), message(output));
    assert!(stderr.lines().all(visible), "{stderr:?}");
    assert!(visible(&message), "{message:?}");
    assert!(message.contains(named), "{message}");
    assert!(
        stderr.contains(&message),
        "{message:?} is not on {stderr:?}"
    );
}

#[test]
fn a_problem_document_s_code_and_detail_are_named_escaped() {
    let failing: fn(&str) -> RemoteScenario =
        |key| RemoteScenario::failing(key, 418, Some(HOSTILE));
    let closed: fn(&str) -> RemoteScenario = |key| RemoteScenario::accepting(key).closed(HOSTILE);
    for (scenario, said) in [
        (failing, format!("418 with the code `{ESCAPED}`")),
        (closed, format!("409 with the code `conflict`: {ESCAPED}")),
    ] {
        let run = connected(1, 1, scenario);
        run.change();
        assert_named(&run.publish(), &said);
    }
}

/// Runs `loft remote login --format json` for a Reading List whose remote is `address`, with the
/// browser simulator visiting each address it is given.
pub fn login_at(address: &str) -> (Scratch, Output) {
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(address), true);
    let args = ["remote", "login", "--format", "json"];
    let (output, _) = run(&scratch, &args, BrowserScenario::Visiting, None);
    (scratch, output)
}

#[test]
fn a_refused_registration_or_code_exchange_names_its_oauth_error_escaped() {
    for scenario in [
        RemoteScenario::accepting("doc-register").refusing_registration(HOSTILE),
        RemoteScenario::accepting("doc-exchange").refusing_exchange(HOSTILE),
    ] {
        let remote = Remote::start(scenario);
        let (scratch, output) = login_at(&remote.address());
        assert_named(&output, &format!("`{ESCAPED}`"));
        assert!(!sign_ins_file(&scratch).exists(), "nothing is written");
    }
}

/// An authorization server that refuses every token request with [`HOSTILE`] as its `error`.
struct RefusingTokens;

impl Responder for RefusingTokens {
    fn answer(&mut self, _: &Request) -> Answer {
        Response::json(
            400,
            &json!({ "error": HOSTILE, "error_description": HOSTILE }),
        )
        .into()
    }
}

#[test]
fn a_refused_refresh_names_its_oauth_error_escaped() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let refusing = Server::start(RefusingTokens);
    let mut kept = granted(&run.remote, 3600);
    kept["tokenEndpoint"] = json!(format!("{}/token", refusing.url()));
    kept["expiresAt"] = json!("2000-01-01T00:00:00Z");
    keep(&run.scratch, &json!({ run.remote.address(): kept }));
    run.change();
    let output = run.publish();
    assert_named(&output, &format!("has lapsed ({ESCAPED})"));
    assert_eq!(refusing.requests().len(), 1, "the refresh was asked for");
}

#[test]
fn a_kept_sign_in_for_a_resource_on_another_origin_names_that_resource_escaped() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let mut kept = granted(&run.remote, 3600);
    let resource = format!("https://elsewhere.example/documents#{HOSTILE}");
    kept["resource"] = json!(resource);
    keep(&run.scratch, &json!({ run.remote.address(): kept }));
    run.change();
    let output = run.publish();
    assert_named(
        &output,
        &format!("https://elsewhere.example/documents#{ESCAPED}"),
    );
    assert!(run.calls().is_empty(), "the sign-in is not sent");
}

/// What a remote of the test's own spoils of a sign-in it otherwise answers as the contract
/// states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spoiled {
    /// Its challenge names protected resource metadata on another origin, whose path ends in
    /// [`TABBED`].
    Challenge,
    /// Its protected resource metadata names a `resource` that ends in [`HOSTILE`].
    Resource,
    /// Its challenge names protected resource metadata whose fragment is [`TABBED`], and that
    /// metadata names its own address without the well-known path as the `resource`.
    ResourceFragment,
    /// Its protected resource metadata names an issuer whose fragment is [`HOSTILE`], which the
    /// authorization server's metadata does not name.
    Issuer,
    /// Its protected resource metadata names an issuer whose fragment is [`HOSTILE`], and the
    /// authorization server's metadata names the same issuer.
    EchoedIssuer,
    /// The authorization server's metadata names an authorization endpoint whose fragment is
    /// [`HOSTILE`].
    Endpoint,
    /// The authorization endpoint sends the browser back with [`HOSTILE`] as the `error`.
    Callback,
}

/// A remote of the test's own, at `/doc` on its origin, that spoils what [`Spoiled`] names.
struct Spoiling(Spoiled);

/// The path of the protected resource metadata of `/doc`.
const RESOURCE_METADATA: &str = "/.well-known/oauth-protected-resource/doc";

impl Spoiling {
    /// `text` where this remote spoils one of `those`, or else nothing.
    fn spoiling<'a>(&self, those: &[Spoiled], text: &'a str) -> &'a str {
        if those.contains(&self.0) { text } else { "" }
    }

    /// The challenge of an unauthenticated request, naming the metadata of `/doc`, or, for
    /// [`Spoiled::Challenge`], metadata on another origin.
    fn challenge(&self, origin: &str) -> Response {
        let mut response = Response::empty(401);
        let metadata = if self.0 == Spoiled::Challenge {
            format!("http://elsewhere.example{RESOURCE_METADATA}/{TABBED}")
        } else {
            let fragment = format!("#{TABBED}");
            let fragment = self.spoiling(&[Spoiled::ResourceFragment], &fragment);
            format!("{origin}{RESOURCE_METADATA}{fragment}")
        };
        let challenge = format!("Bearer resource_metadata=\"{metadata}\"");
        response
            .headers
            .push(("WWW-Authenticate".to_owned(), challenge));
        response
    }

    /// The authorization endpoint's redirect back to the address `asked` names, with its `state`
    /// and the `error` [`HOSTILE`].
    fn refused(asked: &str) -> Response {
        let asked = pairs(asked);
        let back = value(&asked, "redirect_uri").unwrap_or_default();
        let state = value(&asked, "state").unwrap_or_default();
        Response::redirect(&format!(
            "{back}?{}",
            query(&[("state", state), ("error", HOSTILE)])
        ))
    }
}

impl Responder for Spoiling {
    fn answer(&mut self, request: &Request) -> Answer {
        let origin = format!("http://{}", request.header("host").unwrap_or_default());
        let (path, asked) = (request.path.split_once('?')).unwrap_or((&request.path, ""));
        // The address of `at` on the origin, followed by `suffix` where one of `those` is spoiled.
        let address = |at: &str, those: &[Spoiled], suffix: &str| {
            format!("{origin}{at}{}", self.spoiling(those, suffix))
        };
        let (fragment, tabbed) = (format!("#{HOSTILE}"), format!("#{TABBED}"));
        let resource = address("/doc", &[Spoiled::Resource], HOSTILE)
            + self.spoiling(&[Spoiled::ResourceFragment], &tabbed);
        let issued = [Spoiled::Issuer, Spoiled::EchoedIssuer];
        let authorization = address("/authorize", &[Spoiled::Endpoint], &fragment);
        let response = match path {
            "/doc/head" => self.challenge(&origin),
            RESOURCE_METADATA => Response::json(
                200,
                &json!({
                    "resource": resource,
                    "authorization_servers": [address("/auth", &issued, &fragment)],
                    "scopes_supported": ["documents"],
                }),
            ),
            "/.well-known/oauth-authorization-server/auth" => Response::json(
                200,
                &json!({
                    "issuer": address("/auth", &[Spoiled::EchoedIssuer], &fragment),
                    "authorization_endpoint": authorization,
                    "token_endpoint": format!("{origin}/token"),
                    "registration_endpoint": format!("{origin}/register"),
                    "code_challenge_methods_supported": ["S256"],
                }),
            ),
            "/register" => Response::json(201, &json!({ "client_id": "client" })),
            "/authorize" => Self::refused(asked),
            _ => Response::empty(404),
        };
        response.into()
    }
}

#[test]
fn a_sign_in_names_what_its_metadata_or_its_callback_gave_escaped() {
    for (spoiled, said) in [
        (
            Spoiled::Challenge,
            format!("elsewhere.example{RESOURCE_METADATA}/{TABBED_ESCAPED}"),
        ),
        (Spoiled::Resource, format!("/doc{ESCAPED}")),
        (Spoiled::Issuer, format!("/auth#{ESCAPED}")),
        (Spoiled::Endpoint, format!("/authorize#{ESCAPED}")),
        (Spoiled::Callback, format!("answered `{ESCAPED}`")),
    ] {
        let server = Server::start(Spoiling(spoiled));
        let (scratch, output) = login_at(&format!("{}/doc", server.url()));
        assert_named(&output, &said);
        let paths: Vec<String> = (server.requests().into_iter())
            .map(|request| request.path)
            .collect();
        let read = paths.iter().any(|path| path == RESOURCE_METADATA);
        assert_eq!(read, spoiled != Spoiled::Challenge, "{paths:?}");
        let authorized = paths.iter().any(|path| path.starts_with("/authorize"));
        assert_eq!(
            authorized,
            matches!(spoiled, Spoiled::Callback),
            "{paths:?}"
        );
        assert!(
            !sign_ins_file(&scratch).exists(),
            "{spoiled:?}: nothing is written"
        );
    }
}

#[test]
fn a_sign_in_refuses_a_resource_or_an_issuer_its_metadata_names_outside_visible_ascii() {
    for (spoiled, named) in [
        (
            Spoiled::ResourceFragment,
            format!("/doc#{TABBED_ESCAPED}: {RULE}"),
        ),
        (Spoiled::EchoedIssuer, format!("/auth#{ESCAPED}: {RULE}")),
    ] {
        let server = Server::start(Spoiling(spoiled));
        let (scratch, output) = login_at(&format!("{}/doc", server.url()));
        assert_named(&output, &named);
        let paths: Vec<String> = (server.requests().into_iter())
            .map(|request| request.path)
            .collect();
        let asked = |prefix: &str| paths.iter().any(|path| path.starts_with(prefix));
        assert!(
            !asked("/.well-known/oauth-authorization-server") && !asked("/register"),
            "{spoiled:?}: the authorization server is not asked: {paths:?}"
        );
        assert!(
            !sign_ins_file(&scratch).exists(),
            "{spoiled:?}: nothing is written"
        );
    }
}

/// A remote of the test's own that holds a document at publish sequence 1, whose published state
/// it gives with a lock that holds a member named [`HOSTILE`].
struct SpoiledLock;

impl Responder for SpoiledLock {
    fn answer(&mut self, request: &Request) -> Answer {
        let response = match request.path.as_str() {
            "/doc/head" => Response::json(
                200,
                &json!({ "head": { "publishSequence": 1, "stateRoot": "root" } }),
            ),
            "/doc/states/1" => Response::json(200, &json!({ "lock": { HOSTILE: 1 }, "files": [] })),
            _ => Response::empty(404),
        };
        response.into()
    }
}

#[test]
fn a_lock_in_a_remote_s_answer_that_does_not_read_is_named_with_its_reason_escaped() {
    let server = Server::start(SpoiledLock);
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(&format!("{}/doc", server.url())), true);
    let before = tree(&scratch.0);
    let args = ["pull", "--format", "json"];
    let (output, _) = run(&scratch, &args, BrowserScenario::Absent, None);
    assert_named(&output, &format!("`{ESCAPED}`"));
    assert!(message(&output).contains("does not read"), "{output:?}");
    assert_eq!(tree(&scratch.0), before, "nothing is written");
}
