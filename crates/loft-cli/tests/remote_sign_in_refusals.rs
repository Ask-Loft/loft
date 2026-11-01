//! A sign-in that the remote's metadata or its authorization server refuses: metadata that names
//! another resource or issuer, leaves out what a sign-in needs or names an address no remote may
//! have, and a refused registration or code exchange, each run under the remote and browser
//! simulators; and the scope a sign-in asks for.

use software_plant::BrowserScenario;
use software_plant::remote::wire::{pairs, value};
use software_plant::remote::{Metadata, Remote, RemoteScenario};

use crate::common::Scratch;
use crate::common::signed_in::{name_remote, run, sign_ins_file};

/// Runs `loft remote login` for a Reading List connected to a simulator of `scenario`, and gives
/// the simulator, the scratch directory, the output and the addresses the browser was given.
fn login(scenario: RemoteScenario) -> (Remote, Scratch, std::process::Output, Vec<String>) {
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
