//! What `loft interface` says when another document's remote refuses it: an address that may not
//! be a document's remote, given by `--remote` or by the handle's entry of `documents`, and a
//! sign-in the remote refuses. Each error gives what mends it in the words of `loft interface` and
//! of the manifest, and names no `loft remote` command.

use serde_json::json;
use software_plant::remote::{Remote, RemoteScenario};

use crate::common::copies::{HANDLE, OTHER};
use crate::common::others::{list, listing, shelves, shelves_remote};
use crate::common::signed_in::{granted, keep};
use crate::common::{Scratch, stderr};
use crate::interface_add::{conforming, interface, ready};

/// The form of a document's address on a Loft server, with its placeholder for the key.
const FORM: &str = "https://<host>/loft/t/<tenant-slug>/api/v2/documents/<documentKey>/remote";

/// The error message of `loft interface` run with `args`, which must exit 1.
fn refused(scratch: &Scratch, args: &[&str]) -> String {
    let mut all = args.to_vec();
    all.extend(["--format", "json"]);
    let output = interface(scratch, &all);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let result = conforming(&output);
    result["errors"][0]["message"].as_str().unwrap().to_owned()
}

#[test]
fn an_add_at_an_address_that_is_no_document_s_remote_says_that_remote_takes_the_document_s() {
    let history = shelves();
    let remote = shelves_remote(&history);
    let scratch = Scratch::with_reading_list();
    listing(&scratch, &remote, false);
    let message = refused(
        &scratch,
        &["add", "shelves:IF-1", "--remote", remote.origin()],
    );
    let request = format!("GET {}/head answered 404", remote.origin());
    assert!(message.contains(&request), "{message}");
    assert!(
        message.contains("may not be a document's remote"),
        "{message}"
    );
    assert!(message.contains(FORM), "{message}");
    assert!(
        message.contains("`--remote` takes the other document's address"),
        "{message}"
    );
    assert!(!message.contains("loft remote"), "{message}");
}

#[test]
fn a_login_at_a_listed_address_that_is_no_document_s_remote_points_at_its_entry() {
    let history = shelves();
    let remote = shelves_remote(&history);
    let scratch = Scratch::with_reading_list();
    listing(&scratch, &remote, false);
    list(&scratch, remote.origin());
    let message = refused(&scratch, &["login", HANDLE]);
    assert!(
        message.contains("may not be a document's remote"),
        "{message}"
    );
    assert!(
        message.contains(&FORM.replace("<documentKey>", OTHER)),
        "{message}"
    );
    assert!(
        message.contains(&format!("the entry of `documents` for {HANDLE}")),
        "{message}"
    );
    assert!(!message.contains("loft remote"), "{message}");
}

/// A remote of the other document that refuses every request's sign-in.
fn refusing(key: &str) -> Remote {
    Remote::start(RemoteScenario::failing(key, 401, Some("unauthenticated")))
}

/// Asserts that `message` says `loft interface login` signs in again and names no `loft remote`
/// command.
fn assert_login_again(message: &str) {
    let login = format!("`loft interface login {HANDLE}` signs in again");
    assert!(message.contains(&login), "{message}");
    assert!(message.contains("answered 401"), "{message}");
    assert!(!message.contains("loft remote"), "{message}");
}

#[test]
fn an_add_whose_sign_in_the_remote_refuses_says_that_interface_login_signs_in_again() {
    let history = shelves();
    let remote = refusing(&history.key);
    let scratch = Scratch::with_reading_list();
    listing(&scratch, &remote, true);
    assert_login_again(&refused(&scratch, &["add", "shelves:IF-1"]));
}

#[test]
fn a_status_whose_sign_in_the_remote_refuses_says_that_interface_login_signs_in_again() {
    let (scratch, history, remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let refusing = refusing(&history.key);
    scratch.edit("loftspec.yaml", &remote.address(), &refusing.address());
    keep(
        &scratch,
        &json!({ refusing.address(): granted(&refusing, 3600) }),
    );
    for command in ["status", "update"] {
        assert_login_again(&refused(&scratch, &[command]));
    }
}

#[test]
fn a_status_whose_kept_sign_in_has_lapsed_says_that_interface_login_signs_in_again() {
    let history = shelves();
    let scenario = RemoteScenario::accepting(&history.key)
        .holding("Shelves", history.states.clone())
        .lapsing();
    let remote = Remote::start(scenario);
    let scratch = Scratch::with_reading_list();
    listing(&scratch, &remote, true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    keep(&scratch, &json!({ remote.address(): granted(&remote, 60) }));
    let message = refused(&scratch, &["status"]);
    assert!(message.contains("has lapsed"), "{message}");
    let login = format!("`loft interface login {HANDLE}` signs in again");
    assert!(message.contains(&login), "{message}");
    assert!(!message.contains("loft remote"), "{message}");
}
