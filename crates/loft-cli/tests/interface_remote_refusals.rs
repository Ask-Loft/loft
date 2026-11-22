//! What `loft interface` says when another document's remote refuses it: an address that may not
//! be a document's remote, given by `--remote` or by the handle's entry of `documents`; a sign-in
//! the remote refuses, which `add` and `update` make afresh; one that has lapsed or is kept for
//! another origin; and a remote that holds no such document. Each error gives what mends it in
//! the words of `loft interface` and of the manifest, and names no `loft remote` command.

use serde_json::json;
use software_plant::BrowserScenario;
use software_plant::remote::{Remote, RemoteScenario, document_address};

use crate::common::copies::{HANDLE, OTHER};
use crate::common::others::{list, listing, shelves, shelves_remote};
use crate::common::signed_in::{self, granted, keep, sign_ins};
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

/// Keeps a sign-in to `remote` whose access token the remote never granted, so that it refuses
/// it, under `address`.
fn keep_stale(scratch: &Scratch, remote: &Remote, address: &str) {
    let mut stale = granted(remote, 3600);
    stale["accessToken"] = json!("never-granted");
    keep(scratch, &json!({ address: stale }));
}

/// Runs `loft interface` with `args` and a browser that consents, and gives the output and the
/// number of addresses the browser was given.
fn consenting(scratch: &Scratch, args: &[&str]) -> (std::process::Output, usize) {
    let mut all = vec!["interface"];
    all.extend(args);
    let (output, browser) = signed_in::run(scratch, &all, BrowserScenario::Visiting, None);
    (output, browser.visits().len())
}

#[test]
fn an_add_whose_kept_sign_in_the_remote_refuses_signs_in_afresh_once_and_goes_on() {
    for listed in [true, false] {
        let history = shelves();
        let remote = shelves_remote(&history);
        let scratch = Scratch::with_reading_list();
        listing(&scratch, &remote, listed);
        keep_stale(&scratch, &remote, &remote.address());
        let address = remote.address();
        let mut args = vec!["add", "shelves:IF-1"];
        if !listed {
            args.extend(["--remote", &address]);
        }
        let (output, visits) = consenting(&scratch, &args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{listed}: {}",
            stderr(&output)
        );
        assert_eq!(visits, 1, "{listed}: one sign-in");
        let kept = &sign_ins(&scratch)[&address]["accessToken"];
        assert_ne!(kept, "never-granted", "{listed}: the new sign-in is kept");
    }
}

#[test]
fn an_update_whose_kept_sign_in_the_remote_refuses_signs_in_afresh_once_and_goes_on() {
    let (scratch, _history, remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    keep_stale(&scratch, &remote, &remote.address());
    let (output, visits) = consenting(&scratch, &["update"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(visits, 1);
}

#[test]
fn a_status_whose_kept_sign_in_the_remote_refuses_says_that_interface_login_signs_in_again() {
    let (scratch, _history, remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    keep_stale(&scratch, &remote, &remote.address());
    let message = refused(&scratch, &["status"]);
    assert!(message.contains("answered 401"), "{message}");
    let login = format!("`loft interface login {HANDLE}` signs in again");
    assert!(message.contains(&login), "{message}");
    assert!(!message.contains("loft remote"), "{message}");
}

#[test]
fn a_status_whose_sign_in_is_kept_for_another_origin_says_where_the_address_lies() {
    let (scratch, _history, remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let mut elsewhere = granted(&remote, 3600);
    elsewhere["resource"] = json!("https://elsewhere.example/loft/t/plant/api/v2/documents");
    keep(&scratch, &json!({ remote.address(): elsewhere }));
    let message = refused(&scratch, &["status"]);
    assert!(message.contains("another origin"), "{message}");
    assert!(
        message.contains(&format!("the entry of `documents` for {HANDLE}")),
        "{message}"
    );
    let login = format!("`loft interface login {HANDLE}` signs in again once");
    assert!(message.contains(&login), "{message}");
    assert!(!message.contains("loft remote"), "{message}");
}

#[test]
fn an_add_of_an_unlisted_handle_whose_remote_holds_no_such_document_names_remote() {
    let history = shelves();
    let remote = shelves_remote(&history);
    let scratch = Scratch::with_reading_list();
    listing(&scratch, &remote, false);
    let address = document_address(remote.origin(), UNHELD);
    keep(&scratch, &json!({ &address: granted(&remote, 3600) }));
    let message = refused(&scratch, &["add", "shelves:IF-1", "--remote", &address]);
    assert!(message.contains("holds no such document"), "{message}");
    assert!(
        message.contains("the address `--remote` gives"),
        "{message}"
    );
    assert!(!message.contains("`documents`"), "{message}");
}

/// The key of a document the other document's remote does not hold.
const UNHELD: &str = "00000000-0000-4000-8000-000000000999";

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
    let args = ["interface", "status", "--format", "json"];
    let (output, browser) = signed_in::run(&scratch, &args, BrowserScenario::Visiting, None);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let message = conforming(&output)["errors"][0]["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        browser.visits(),
        Vec::<String>::new(),
        "no browser is opened"
    );
    assert!(message.contains("has lapsed"), "{message}");
    let login = format!("`loft interface login {HANDLE}` signs in again");
    assert!(message.contains(&login), "{message}");
    assert!(!message.contains("loft remote"), "{message}");
}
