//! `loft pull` with no sign-in: a document its remote serves to anyone is pulled without a token,
//! and a remote that refuses such a request is answered with the error for no sign-in, with
//! nothing written; a kept sign-in is still sent.

use std::fs;

use serde_json::json;
use software_plant::remote::{RemoteScenario, Visibility, document_path};

use crate::common::connected::Connected;
use crate::common::remote::{History, history};
use crate::common::signed_in::{document, keep, sign_ins_file};
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

/// Each request under the document's address, by its path below it, and whether it carried a
/// token.
fn requests(run: &Connected) -> Vec<(String, bool)> {
    let document = document_path(&run.history.key);
    run.remote
        .requests()
        .into_iter()
        .filter_map(|request| {
            let below = request.path.strip_prefix(&document)?.to_owned();
            Some((below, request.header("Authorization").is_some()))
        })
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
    let message = document(&output)["errors"][0]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let address = run.remote.address();
    for said in [
        run.history.key.as_str(),
        address.as_str(),
        "no sign-in",
        "LOFT_REMOTE_TOKEN",
        "`loft remote login` signs in",
    ] {
        assert!(message.contains(said), "{said}: {message}");
    }
    assert_eq!(requests(&run), [("/head".to_owned(), false)]);
    assert_eq!(tree(&run.scratch.0), before);
    assert_eq!(fs::read(sign_ins_file(&run.scratch)).unwrap(), file);
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
