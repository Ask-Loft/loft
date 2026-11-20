//! A sign-in that the remote's metadata or its authorization server refuses: metadata that names
//! another resource or issuer, leaves out what a sign-in needs or names an address no remote may
//! have, and a refused registration or code exchange, each run under the remote and browser
//! simulators; an address that answers the first request with no challenge; and the scope a
//! sign-in asks for.

use software_plant::BrowserScenario;
use software_plant::http::{Answer, Request, Responder, Response, Server};
use software_plant::remote::wire::{pairs, value};
use software_plant::remote::{Metadata, Remote, RemoteScenario, document_address};

use crate::common::Scratch;
use crate::common::remote::history;
use crate::common::signed_in::{name_remote, run, sign_ins_file};
use crate::remote_connect::{assert_nothing_written, published};

/// Runs `loft remote login` for a Reading List connected to a simulator of `scenario`, and gives
/// the simulator, the scratch directory, the output and the addresses the browser was given.
pub fn login(scenario: RemoteScenario) -> (Remote, Scratch, std::process::Output, Vec<String>) {
    let remote = Remote::start(scenario);
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(&remote.address()), true);
    let (output, browser) = run(
        &scratch,
        &["remote", "login"],
        BrowserScenario::Visiting,
        None,
    );
    (remote, scratch, output, browser.visits())
}

#[test]
fn metadata_that_misnames_itself_or_lacks_what_a_sign_in_needs_ends_the_sign_in() {
    for (metadata, said) in [
        (Metadata::MisnamedResource, "names another resource"),
        (Metadata::MisnamedIssuer, "names another issuer"),
        (Metadata::Unregistering, "names no `registration_endpoint`"),
        (Metadata::Plain, "does not take PKCE with S256"),
        (Metadata::Unencrypted, "http://auth.example/authorize"),
    ] {
        let (remote, scratch, output, visits) =
            login(RemoteScenario::accepting("doc-metadata").metadata(metadata.clone()));
        assert_eq!(output.status.code(), Some(1), "{metadata:?}: {output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(said), "{metadata:?}: {stderr}");
        assert_eq!(visits, Vec::<String>::new(), "{metadata:?}: no browser");
        assert!(
            remote.requests().iter().all(|r| r.path != "/auth/register"),
            "{metadata:?}: no client is registered"
        );
        assert!(!sign_ins_file(&scratch).exists(), "{metadata:?}");
    }
}

#[test]
fn the_scope_asked_for_is_the_challenge_s_or_else_every_scope_the_metadata_lists() {
    for (scenario, scope) in [
        (RemoteScenario::accepting("doc-scope"), "documents"),
        (
            RemoteScenario::accepting("doc-scopes").metadata(Metadata::Unscoped(vec![
                "documents".into(),
                "profile".into(),
            ])),
            "documents profile",
        ),
    ] {
        let (_remote, _scratch, output, visits) = login(scenario);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let query = visits[0].split_once('?').unwrap().1;
        assert_eq!(value(&pairs(query), "scope"), Some(scope));
    }
}

#[test]
fn a_refused_registration_or_exchange_ends_the_sign_in_naming_its_oauth_error() {
    for (scenario, error, visited) in [
        (
            RemoteScenario::accepting("doc-register")
                .refusing_registration("invalid_client_metadata"),
            "invalid_client_metadata",
            0,
        ),
        (
            RemoteScenario::accepting("doc-exchange").refusing_exchange("invalid_grant"),
            "invalid_grant",
            1,
        ),
    ] {
        let (_remote, scratch, output, visits) = login(scenario);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(error), "{stderr}");
        assert_eq!(visits.len(), visited);
        assert!(!sign_ins_file(&scratch).exists(), "nothing is written");
    }
}

/// The form of a document's address on a Loft server, as a refusal gives it.
const FORM: &str = "https://<host>/loft/t/<tenant-slug>/api/v2/documents/<documentKey>/remote";

#[test]
fn an_unchallenged_answer_names_the_request_and_outside_a_401_gives_the_form_of_an_address() {
    for status in [401, 404, 502] {
        let (remote, scratch, output, visits) =
            login(RemoteScenario::failing("doc-unchallenged", status, None));
        assert_eq!(output.status.code(), Some(1), "{status}: {output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let request = format!("GET {}/head answered {status}", remote.address());
        assert!(stderr.contains(&request), "{status}: {stderr}");
        for hint in [
            "may not be a document's remote",
            FORM,
            "`documentKey` of spec/loftspec.yaml",
            "`loft remote set` with the document's address replaces",
        ] {
            assert_eq!(
                stderr.contains(hint),
                status != 401,
                "{status}: {hint}: {stderr}"
            );
        }
        assert_eq!(visits, Vec::<String>::new(), "{status}: no browser");
        assert!(!sign_ins_file(&scratch).exists(), "{status}");
    }
}

#[test]
fn a_connect_given_the_server_s_origin_alone_names_the_request_and_the_form() {
    let history = history(1);
    let (scratch, git) = published(&history);
    let remote = Remote::start(RemoteScenario::accepting(&history.key));
    let (output, browser) = run(
        &scratch,
        &["remote", "set", remote.origin()],
        BrowserScenario::Visiting,
        Some(&git),
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("GET {}/head answered 404", remote.origin())),
        "{stderr}"
    );
    assert!(stderr.contains(FORM), "{stderr}");
    assert_eq!(browser.visits(), Vec::<String>::new());
    assert_nothing_written(&scratch);
}

/// A remote that challenges every request as `Bearer` without naming its metadata.
struct Unnamed;

impl Responder for Unnamed {
    fn answer(&mut self, _: &Request) -> Answer {
        let mut response = Response::empty(401);
        response
            .headers
            .push(("WWW-Authenticate".into(), "Bearer realm=\"loft\"".into()));
        response.into()
    }
}

#[test]
fn a_bearer_challenge_that_names_no_metadata_is_refused_naming_the_request() {
    let server = Server::start(Unnamed);
    let address = document_address(&server.url(), "doc-unnamed");
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(&address), true);
    let (output, browser) = run(
        &scratch,
        &["remote", "login"],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("GET {address}/head answered 401")),
        "{stderr}"
    );
    assert!(stderr.contains("names no `resource_metadata`"), "{stderr}");
    assert!(!stderr.contains(FORM), "{stderr}");
    assert_eq!(browser.visits(), Vec::<String>::new());
    assert!(!sign_ins_file(&scratch).exists());
}
