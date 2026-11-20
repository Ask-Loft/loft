//! A sign-in that the remote's metadata or its authorization server refuses: a challenge that
//! names metadata on another origin, metadata that names a resource on another origin, another
//! resource or issuer, leaves out what a sign-in needs or names an address no remote may
//! have, and a refused registration or code exchange, each run under the remote and browser
//! simulators; an address that answers the first request with no challenge; and the scope a
//! sign-in asks for.

use software_plant::BrowserScenario;
use software_plant::http::{Answer, Request, Responder, Response, Server};
use software_plant::remote::wire::{pairs, value};
use software_plant::remote::{Metadata, Remote, RemoteScenario, document_address, document_path};

use crate::common::calls::call;
use crate::common::oauth::{named_elsewhere, parameter};
use crate::common::remote::history;
use crate::common::signed_in::{name_remote, run, sign_ins_file};
use crate::common::{Scratch, tree};
use crate::remote_connect::{assert_nothing_written, published};
use crate::schema;

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

/// The form of a document's address on a Loft server, with its placeholder for the key.
const FORM: &str = "https://<host>/loft/t/<tenant-slug>/api/v2/documents/<documentKey>/remote";

/// The form of the address of the document that `scratch`'s manifest names.
fn form_for(scratch: &Scratch) -> String {
    let key = loftspec::codec::read(&tree(&scratch.0))
        .unwrap()
        .id
        .key()
        .to_string();
    FORM.replace("<documentKey>", &key)
}

#[test]
fn only_an_answer_a_wrong_address_gives_brings_the_form_with_the_manifest_s_key() {
    for status in [401, 404, 405, 408, 429, 502, 503] {
        let (remote, scratch, output, visits) =
            login(RemoteScenario::failing("doc-unchallenged", status, None));
        assert_eq!(output.status.code(), Some(1), "{status}: {output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let request = format!("GET {}/head answered {status}", remote.address());
        assert!(stderr.contains(&request), "{status}: {stderr}");
        let wrong = matches!(status, 404 | 405);
        for hint in [
            "may not be a document's remote".to_owned(),
            form_for(&scratch),
            "`loft remote set` with the document's address replaces".to_owned(),
        ] {
            assert_eq!(stderr.contains(&hint), wrong, "{status}: {hint}: {stderr}");
        }
        assert!(!stderr.contains(FORM), "{status}: no placeholder: {stderr}");
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
    assert!(stderr.contains(&form_for(&scratch)), "{stderr}");
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

#[test]
fn a_challenge_naming_metadata_on_another_origin_is_refused_before_that_metadata_is_requested() {
    let remote = Remote::start(
        RemoteScenario::accepting("doc-elsewhere").metadata(Metadata::OnAnotherOrigin),
    );
    let address = remote.address();
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(&address), true);
    let before = tree(&scratch.0);
    let (output, browser) = run(
        &scratch,
        &["remote", "login", "--format", "json"],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    // The challenge alone was requested: no metadata, no authorization server, no registration.
    let requested: Vec<String> = remote.requests().into_iter().map(|r| r.path).collect();
    assert_eq!(
        requested,
        [format!("{}/head", document_path("doc-elsewhere"))]
    );
    let elsewhere = remote.requests_elsewhere();
    assert!(
        elsewhere.is_empty(),
        "the other origin is asked nothing: {elsewhere:?}"
    );
    assert_eq!(
        browser.visits(),
        Vec::<String>::new(),
        "no browser is opened"
    );
    assert!(!sign_ins_file(&scratch).exists(), "no sign-in is kept");
    assert_eq!(tree(&scratch.0), before, "no file of the tree is written");
    let result = schema::conforming(&output.stdout, include_str!("../../../schemas/remote.json"));
    assert_eq!(result["outcome"], "error");
    assert_eq!(result["errors"][0]["kind"], "remote");
    let message = result["errors"][0]["message"].as_str().unwrap();
    let challenge = call("GET", &format!("{address}/head"), None, None);
    let header = challenge.header("www-authenticate").unwrap();
    let named = parameter(header, "resource_metadata").unwrap();
    for part in [address.as_str(), named.as_str(), "another origin"] {
        assert!(message.contains(part), "{part}: {message}");
    }
    assert!(String::from_utf8_lossy(&output.stderr).contains(message));
}

#[test]
fn metadata_naming_a_resource_on_another_origin_is_refused_before_a_client_is_registered() {
    let remote = Remote::start(
        RemoteScenario::accepting("doc-named-elsewhere")
            .metadata(Metadata::ResourceOnAnotherOrigin),
    );
    let address = remote.address();
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(&address), true);
    let before = tree(&scratch.0);
    let (output, browser) = run(
        &scratch,
        &["remote", "login", "--format", "json"],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let requested: Vec<String> = remote
        .requests()
        .into_iter()
        .map(|request| format!("{} {}", request.method, request.path))
        .collect();
    // The challenge and the metadata it names alone were requested: no authorization server's
    // metadata, no registration.
    let (metadata, refused) = named_elsewhere(&remote);
    let head = format!("GET {}/head", document_path("doc-named-elsewhere"));
    assert_eq!(requested, [head, format!("GET {metadata}")]);
    let elsewhere = remote.requests_elsewhere();
    assert!(
        elsewhere.is_empty(),
        "the other origin is asked nothing: {elsewhere:?}"
    );
    assert_eq!(
        browser.visits(),
        Vec::<String>::new(),
        "no browser is opened"
    );
    assert!(!sign_ins_file(&scratch).exists(), "no sign-in is kept");
    assert_eq!(tree(&scratch.0), before, "no file of the tree is written");
    let result = schema::conforming(&output.stdout, include_str!("../../../schemas/remote.json"));
    assert_eq!(result["outcome"], "error");
    assert_eq!(result["errors"][0]["kind"], "remote");
    let message = result["errors"][0]["message"].as_str().unwrap();
    for part in [address.as_str(), refused.as_str(), "another origin"] {
        assert!(message.contains(part), "{part}: {message}");
    }
    assert!(String::from_utf8_lossy(&output.stderr).contains(message));
}
