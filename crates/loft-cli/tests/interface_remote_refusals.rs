//! What `loft interface` says when another document's remote refuses it: an address that may not
//! be a document's remote, given by `--remote` or by the handle's entry of `documents`. Each error
//! gives what mends it in the words of `loft interface` and of the manifest, and names no
//! `loft remote` command.

use crate::common::copies::{HANDLE, OTHER};
use crate::common::others::{list, listing, shelves, shelves_remote};
use crate::common::{Scratch, stderr};
use crate::interface_add::{conforming, interface};

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
