//! `loft publish` of a connected document's first publish: before it sends the publish, a line on
//! standard error says to whom the remote serves the document, where the remote's head answer names
//! it; nothing is said of it for a later publish, and the output is as it is without the line.

use std::process::Output;

use software_plant::remote::{RemoteScenario, Visibility};
use software_plant::{BrowserScenario, GitScenario};

use crate::common::connected::{connected, serve};
use crate::common::signed_in::{self, document};
use crate::common::{Scratch, tree};

/// The line said before the first publish of a document the remote serves to anyone.
pub const PUBLIC: &str = "the remote serves this document to anyone: anyone reads its published states without signing in";

/// The line said before the first publish of a document the remote serves to those it admits.
pub const PRIVATE: &str = "the remote serves this document privately: only those it admits read it";

/// The first publish of the Reading List to a remote that holds it with no published state, whose
/// scenario names `visibility`.
fn first_publish(visibility: Option<Visibility>) -> Output {
    let scratch = Scratch::with_reading_list();
    let key = loftspec::codec::read(&tree(&scratch.0))
        .unwrap()
        .id
        .key()
        .to_string();
    let scenario = RemoteScenario::accepting(&key).holding("Reading List", Vec::new());
    let _remote = serve(
        &scratch,
        match visibility {
            Some(visibility) => scenario.visibility(visibility),
            None => scenario,
        },
    );
    let git = GitScenario::clean_on("main");
    let (mut command, _) = signed_in::command(&scratch, BrowserScenario::Absent, Some(&git));
    command
        .args(["publish", "--format", "json"])
        .output()
        .unwrap()
}

#[test]
fn a_first_publish_says_to_whom_the_remote_serves_the_document() {
    let unnamed = first_publish(None);
    for (visibility, said) in [(Visibility::Public, PUBLIC), (Visibility::Private, PRIVATE)] {
        let output = first_publish(Some(visibility));
        assert_eq!(output.status.code(), Some(0), "{visibility:?}: {output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(said), "{visibility:?}: {stderr}");
        assert_eq!(document(&output), document(&unnamed), "{visibility:?}");
    }
}

#[test]
fn a_first_publish_to_a_remote_that_names_no_visibility_says_nothing_of_it() {
    let output = first_publish(None);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("the remote serves this document"),
        "{stderr}"
    );
}

#[test]
fn a_publish_to_a_remote_that_holds_a_head_says_nothing_of_the_visibility() {
    let run = connected(1, 1, |key| {
        RemoteScenario::accepting(key).visibility(Visibility::Public)
    });
    run.change();
    let output = run.publish();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("the remote serves this document"),
        "{stderr}"
    );
}
