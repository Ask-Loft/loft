//! `loft remote set` of a document the repository has published: before it imports the published
//! states, it asks the remote for its head and says on standard error to whom the remote serves the
//! document, where the head answer names it; a connect that imports nothing says nothing of it.

use software_plant::remote::{Remote, RemoteScenario, Visibility, document_path};

use crate::common::remote::history;
use crate::publish_notice::{PRIVATE, PUBLIC};
use crate::remote_connect::{codec_key, document_calls, published, set};

/// The words every line on the visibility begins with.
const SAID: &str = "the remote serves this document";

#[test]
fn a_connect_that_imports_says_to_whom_the_remote_serves_the_document_after_asking_its_head() {
    for (visibility, said) in [(Visibility::Public, PUBLIC), (Visibility::Private, PRIVATE)] {
        let history = history(2);
        let (scratch, git) = published(&history);
        let remote = Remote::start(RemoteScenario::accepting(&history.key).visibility(visibility));
        let (output, result) = set(&scratch, &remote, Some(&git));
        assert_eq!(output.status.code(), Some(0), "{visibility:?}: {output:?}");
        assert_eq!(result["outcome"], "connected");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(said), "{visibility:?}: {stderr}");
        let document = document_path(&history.key);
        assert_eq!(
            document_calls(&remote),
            [
                format!("POST {document}/create"),
                format!("GET {document}/head"),
                format!("POST {document}/import"),
            ],
            "{visibility:?}"
        );
    }
}

#[test]
fn a_connect_to_a_remote_that_names_no_visibility_says_nothing_of_it() {
    let history = history(2);
    let (scratch, git) = published(&history);
    let remote = Remote::start(RemoteScenario::accepting(&history.key));
    let (output, _) = set(&scratch, &remote, Some(&git));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains(SAID), "{stderr}");
}

#[test]
fn a_connect_that_imports_nothing_says_nothing_of_the_visibility() {
    let scratch = crate::common::Scratch::with_reading_list();
    let key = codec_key(&scratch);
    let remote = Remote::start(RemoteScenario::accepting(&key).visibility(Visibility::Public));
    let (output, _) = set(&scratch, &remote, None);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains(SAID), "{stderr}");
    assert_eq!(
        document_calls(&remote),
        [format!("POST {}/create", document_path(&key))]
    );
}
