//! `loft pull` with no sign-in: a document its remote serves to anyone is pulled without a token,
//! and a remote that refuses such a request is answered with the error for no sign-in, with
//! nothing written; a kept sign-in is still sent.

use std::fs;
use std::process::Command;

use serde_json::json;
use software_plant::BrowserScenario;
use software_plant::remote::{RemoteScenario, Visibility};

use crate::common::connected::Connected;
use crate::common::remote::{History, history, sent_to};
use crate::common::signed_in::{self, document, granted, keep, kept_from, sign_ins_file};
use crate::common::tree;
use crate::pull::{behind, pull};

/// A repository that published two states, connected to a remote holding three, whose scenario
/// names `visibility`, with a sign-in to it kept where `signed_in` says.
fn behind_one(visibility: Visibility, signed_in: bool) -> (Connected, History) {
    let (run, _) = behind();
    let ahead = history(3);
    let run = run.serving(
        RemoteScenario::accepting(&ahead.key)
            .holding("Reading List", ahead.states.clone())
            .visibility(visibility),
    );
    if !signed_in {
        keep(&run.scratch, &json!({}));
    }
    (run, ahead)
}

/// Each request under the document's address, by its path below it and whether it carried a
/// token.
pub fn requests(run: &Connected) -> Vec<(String, bool)> {
    (sent_to(&run.remote, &run.history.key).into_iter())
        .map(|sent| (sent.path, sent.token))
        .collect()
}

#[test]
fn a_pull_with_no_sign_in_reads_a_public_document_without_a_token() {
    let (run, ahead) = behind_one(Visibility::Public, false);
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result = document(&output);
    assert_eq!(result["outcome"], "pulled");
    assert_eq!(result["publishSequence"], 3);
    assert_eq!(result["stateRoot"], ahead.states[2].lock["stateRoot"]);
    let sent = requests(&run);
    let paths: Vec<&str> = sent.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(paths, ["/head", "/states/2", "/states/3"]);
    assert!(sent.iter().all(|(_, token)| !token), "{sent:?}");
}

#[test]
fn a_pull_with_no_sign_in_that_the_remote_refuses_is_refused_with_the_error_for_no_sign_in() {
    let (run, _) = behind_one(Visibility::Private, false);
    let before = tree(&run.scratch.0);
    let file = fs::read(sign_ins_file(&run.scratch)).unwrap();
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = Connected::message(&output);
    assert_unsigned_refusal(&run, &message);
    for said in ["no sign-in", "`loft remote login` signs in"] {
        assert!(message.contains(said), "{said}: {message}");
    }
    assert_eq!(requests(&run), [("/head".to_owned(), false)]);
    assert_eq!(tree(&run.scratch.0), before);
    assert_eq!(fs::read(sign_ins_file(&run.scratch)).unwrap(), file);
}

/// Asserts that `message`, the error of a pull that the remote refused without a token, names
/// the refusal's status and code, the address, `LOFT_REMOTE_TOKEN` and the document key apart from
/// the address, which holds the key as a Loft server's addresses do.
fn assert_unsigned_refusal(run: &Connected, message: &str) {
    let address = run.remote.address();
    for said in [
        address.as_str(),
        "401",
        "`unauthenticated`",
        "without a token",
        "LOFT_REMOTE_TOKEN",
    ] {
        assert!(message.contains(said), "{said}: {message}");
    }
    let key = run.history.key.as_str();
    assert!(address.contains(key), "{address}");
    let beside = message.replace(address.as_str(), "");
    assert!(
        beside.contains(key),
        "{key} apart from the address: {message}"
    );
}

#[test]
fn a_pull_with_a_kept_sign_in_sends_its_token_to_a_public_document() {
    let (run, _) = behind_one(Visibility::Public, true);
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let sent = requests(&run);
    assert_eq!(sent.len(), 3, "{sent:?}");
    assert!(sent.iter().all(|(_, token)| *token), "{sent:?}");
}

/// A repository that published two states, connected to a remote holding three, whose scenario
/// names `visibility`, with a sign-in kept that expires within a minute and whose refresh the
/// remote refuses.
fn lapsed(visibility: Visibility) -> Connected {
    let (run, _) = behind();
    let ahead = history(3);
    let run = run.serving(
        RemoteScenario::accepting(&ahead.key)
            .holding("Reading List", ahead.states.clone())
            .visibility(visibility)
            .lapsing(),
    );
    let kept = granted(&run.remote, 60);
    keep(&run.scratch, &json!({ run.remote.address(): kept }));
    run
}

#[test]
fn a_pull_whose_sign_in_has_lapsed_reads_a_public_document_without_a_token() {
    let run = lapsed(Visibility::Public);
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(document(&output)["outcome"], "pulled");
    let sent = requests(&run);
    assert_eq!(sent.len(), 3, "{sent:?}");
    assert!(sent.iter().all(|(_, token)| !token), "{sent:?}");
}

#[test]
fn a_pull_whose_sign_in_has_lapsed_that_the_remote_refuses_names_the_lapsed_sign_in() {
    let run = lapsed(Visibility::Private);
    let before = tree(&run.scratch.0);
    let file = fs::read(sign_ins_file(&run.scratch)).unwrap();
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = Connected::message(&output);
    assert_unsigned_refusal(&run, &message);
    for said in ["lapsed", "`loft remote login` signs in again"] {
        assert!(message.contains(said), "{said}: {message}");
    }
    assert!(!message.contains("no sign-in"), "{message}");
    assert_eq!(requests(&run), [("/head".to_owned(), false)]);
    assert_eq!(tree(&run.scratch.0), before);
    assert_eq!(fs::read(sign_ins_file(&run.scratch)).unwrap(), file);
}

/// A repository that published two states, connected to a remote holding three, whose scenario
/// names `visibility`, with a sign-in kept that expires within a minute and was made for a
/// protected resource on the remote's origin that it no longer serves, so that the remote refuses
/// its refresh with `invalid_target`.
fn made_for_former(visibility: Visibility) -> Connected {
    let (run, _) = behind();
    let ahead = history(3);
    let run = run.serving(
        RemoteScenario::accepting(&ahead.key)
            .holding("Reading List", ahead.states.clone())
            .visibility(visibility),
    );
    let former = format!("{}/former", run.remote.origin());
    let grant = run.remote.grant_over(&[&former]);
    let kept = kept_from(&run.remote, &former, &grant, 60);
    keep(&run.scratch, &json!({ run.remote.address(): kept }));
    run
}

#[test]
fn a_pull_whose_sign_in_was_made_for_a_former_resource_reads_a_public_document_without_a_token() {
    let run = made_for_former(Visibility::Public);
    let file = fs::read(sign_ins_file(&run.scratch)).unwrap();
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(document(&output)["outcome"], "pulled");
    let sent = requests(&run);
    assert_eq!(sent.len(), 3, "{sent:?}");
    assert!(sent.iter().all(|(_, token)| !token), "{sent:?}");
    assert_eq!(fs::read(sign_ins_file(&run.scratch)).unwrap(), file);
}

#[test]
fn a_pull_whose_sign_in_was_made_for_a_former_resource_that_the_remote_refuses_names_that_sign_in()
{
    let run = made_for_former(Visibility::Private);
    let before = tree(&run.scratch.0);
    let file = fs::read(sign_ins_file(&run.scratch)).unwrap();
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = Connected::message(&output);
    assert_unsigned_refusal(&run, &message);
    let former = format!("{}/former", run.remote.origin());
    for said in [
        "former resource",
        former.as_str(),
        "`loft remote login` signs in again",
    ] {
        assert!(message.contains(said), "{said}: {message}");
    }
    for unsaid in ["no sign-in", "lapsed"] {
        assert!(!message.contains(unsaid), "{unsaid}: {message}");
    }
    assert_eq!(requests(&run), [("/head".to_owned(), false)]);
    assert_eq!(tree(&run.scratch.0), before);
    assert_eq!(fs::read(sign_ins_file(&run.scratch)).unwrap(), file);
}

/// `command` with no configuration directory on any platform: no home, no `XDG_CONFIG_HOME` and
/// no `APPDATA`.
pub fn homeless(command: &mut Command) -> &mut Command {
    for name in ["HOME", "USERPROFILE", "XDG_CONFIG_HOME", "APPDATA"] {
        command.env_remove(name);
    }
    command
}

#[test]
fn a_pull_without_a_configuration_directory_reads_a_public_document_without_a_token() {
    let (run, ahead) = behind_one(Visibility::Public, false);
    let (mut command, _) = signed_in::command(&run.scratch, BrowserScenario::Absent, None);
    let output = homeless(&mut command)
        .args(["pull", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result = document(&output);
    assert_eq!(result["outcome"], "pulled");
    assert_eq!(result["stateRoot"], ahead.states[2].lock["stateRoot"]);
    let sent = requests(&run);
    assert_eq!(sent.len(), 3, "{sent:?}");
    assert!(sent.iter().all(|(_, token)| !token), "{sent:?}");
}
