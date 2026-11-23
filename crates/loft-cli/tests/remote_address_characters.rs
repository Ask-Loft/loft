//! A remote's address holds visible ASCII alone. An address that holds another character, whether
//! an escape of the manifest's double-quoted `remote` decodes into it or `LOFT_REMOTE_ENDPOINT`,
//! `loft remote set` or an entry of `documents` gives it, is refused before any request, its
//! error naming it escaped; `loft remote remove`, which reaches no remote, still disconnects the
//! document, naming the address escaped. A backslash, which visible ASCII allows, is named
//! doubled wherever a refusal of another document's remote names its address. Each runs under the
//! remote and browser simulators.

use loftspec::codec::{self, MANIFEST};
use serde_json::json;
use software_plant::BrowserScenario;
use software_plant::remote::{Remote, RemoteScenario, StoredState, document_address};

use crate::common::copies::{HANDLE, OTHER};
use crate::common::others::{OWN_REMOTE, list, listing, shelves};
use crate::common::remote::history;
use crate::common::signed_in::{self, granted, keep, name_remote, run, sign_ins_file};
use crate::common::{Scratch, stdout, tree};
use crate::remote_connect::assert_nothing_written;
use crate::remote_escapes::{assert_named, visible};

/// The address the manifest names before a test writes another in its place.
const PLACEHOLDER: &str = "https://placeholder.example/documents/one";

/// What each test appends to an address: as an escape of a double-quoted scalar of the manifest
/// writes it, as the address holds it once that escape is decoded, and as an error names it.
/// Control characters in the fragment, a control character beyond ASCII in the path, a letter
/// beyond ASCII, which is named as it is, and a bidirectional control.
const SPOILS: [(&str, &str, &str); 4] = [
    ("#\\e]0;x\\a", "#\u{1b}]0;x\u{7}", "#\\u{1b}]0;x\\u{7}"),
    ("/\\x9b2J", "/\u{9b}2J", "/\\u{9b}2J"),
    ("/b\\u00fccher", "/b\u{fc}cher", "/b\u{fc}cher"),
    ("#top\\u202e", "#top\u{202e}", "#top\\u{202e}"),
];

/// What the error says of an address that holds a character outside visible ASCII.
pub const RULE: &str = "a remote's address holds visible ASCII characters alone, with every other \
     character percent-encoded";

/// A Reading List connected to its remote, the manifest naming it as `address` followed by
/// `escape`, an escape of the double-quoted scalar the manifest writes the address in.
fn named(address: &str, escape: &str) -> Scratch {
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(PLACEHOLDER), true);
    let written = format!("\"{address}{escape}\"");
    scratch.edit(MANIFEST, &format!("\"{PLACEHOLDER}\""), &written);
    scratch
}

#[test]
fn a_manifest_address_that_decodes_outside_visible_ascii_is_refused_before_any_request() {
    let remote = Remote::start(RemoteScenario::accepting("doc-characters"));
    let address = remote.address();
    for (escape, decoded, escaped) in SPOILS {
        let scratch = named(&address, escape);
        let read = codec::read(&tree(&scratch.0)).unwrap();
        assert_eq!(read.remote, Some(format!("{address}{decoded}")), "{escape}");
        let before = tree(&scratch.0);
        let args = ["pull", "--format", "json"];
        let (output, _) = run(&scratch, &args, BrowserScenario::Absent, None);
        assert_named(&output, &format!("{address}{escaped}: {RULE}"));
        assert_eq!(tree(&scratch.0), before, "{escape}: nothing is written");
    }
    assert!(remote.requests().is_empty(), "no connection is opened");
}

#[test]
fn an_address_that_loft_remote_endpoint_names_is_held_to_visible_ascii() {
    let remote = Remote::start(RemoteScenario::accepting("doc-endpoint"));
    let address = remote.address();
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(&address), true);
    for (_, decoded, escaped) in SPOILS {
        let (mut command, _) = signed_in::command(&scratch, BrowserScenario::Absent, None);
        let output = command
            .args(["pull", "--format", "json"])
            .env("LOFT_REMOTE_ENDPOINT", format!("{address}{decoded}"))
            .env("LOFT_REMOTE_TOKEN", "token")
            .output()
            .unwrap();
        assert_named(&output, &format!("{address}{escaped}: {RULE}"));
    }
    assert!(remote.requests().is_empty(), "no connection is opened");
}

#[test]
fn an_address_given_to_loft_remote_set_is_held_to_visible_ascii() {
    let remote = Remote::start(RemoteScenario::accepting("doc-set"));
    let address = remote.address();
    let scratch = Scratch::with_reading_list();
    for (_, decoded, escaped) in SPOILS {
        let given = format!("{address}{decoded}");
        let args = ["remote", "set", &given, "--format", "json"];
        let (output, browser) = run(&scratch, &args, BrowserScenario::Visiting, None);
        assert_named(&output, &format!("{address}{escaped}: {RULE}"));
        assert_eq!(browser.visits(), Vec::<String>::new(), "no browser");
        assert_nothing_written(&scratch);
    }
    assert!(remote.requests().is_empty(), "no connection is opened");
}

#[test]
fn an_address_an_entry_of_documents_lists_is_held_to_visible_ascii_and_named_escaped() {
    let remote = Remote::start(RemoteScenario::accepting("doc-listed"));
    let address = remote.address();
    for (_, decoded, escaped) in SPOILS {
        let scratch = Scratch::with_reading_list();
        name_remote(&scratch, Some(OWN_REMOTE), true);
        list(&scratch, &format!("{address}{decoded}"));
        let before = tree(&scratch.0);
        let args = ["interface", "login", HANDLE, "--format", "json"];
        let (output, browser) = run(&scratch, &args, BrowserScenario::Visiting, None);
        assert_named(&output, &format!("{address}{escaped}: {RULE}"));
        assert_eq!(browser.visits(), Vec::<String>::new(), "no browser");
        assert!(!sign_ins_file(&scratch).exists(), "no sign-in is kept");
        let other = format!("{address}/other");
        let added = format!("{HANDLE}:IF-1");
        let args = [
            "interface",
            "add",
            &added,
            "--remote",
            &other,
            "--format",
            "json",
        ];
        let (output, _) = run(&scratch, &args, BrowserScenario::Absent, None);
        let listed = format!("`documents` lists {HANDLE} at {address}{escaped}, not {other}");
        assert_named(&output, &listed);
        assert_eq!(tree(&scratch.0), before, "nothing is written");
    }
    assert!(remote.requests().is_empty(), "no connection is opened");
}

#[test]
fn remove_disconnects_a_document_whose_address_holds_such_a_character_naming_it_escaped() {
    let address = "https://remote.example/documents/one";
    for (escape, _, escaped) in SPOILS {
        let scratch = named(address, escape);
        let key = codec::read(&tree(&scratch.0)).unwrap().id.key().to_string();
        let (output, _) = run(
            &scratch,
            &["remote", "remove"],
            BrowserScenario::Absent,
            None,
        );
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let stdout = stdout(&output);
        assert!(stdout.lines().all(visible), "{stdout:?}");
        let line = format!("disconnected the document {key} from {address}{escaped}\n");
        assert!(stdout.starts_with(&line), "{stdout}");
        let document = codec::read(&tree(&scratch.0)).unwrap();
        assert!(!document.connected && document.remote.is_none(), "{escape}");
    }
}

/// A remote of the other document that holds `states` under the key `key` followed by a
/// backslash, which visible ASCII allows in an address.
fn backslashed(key: &str, name: &str, states: &[StoredState]) -> Remote {
    let scenario = RemoteScenario::accepting(&format!("{key}\\x"));
    Remote::start(scenario.holding(name, states.to_vec()))
}

/// A Reading List that [`listing`] connects, listing `remote` under [`HANDLE`] where `listed`.
fn listing_one(remote: &Remote, listed: bool) -> Scratch {
    let scratch = Scratch::with_reading_list();
    listing(&scratch, remote, listed);
    scratch
}

/// `address` as an error names it, each backslash doubled.
fn doubled(address: &str) -> String {
    address.replace('\\', "\\\\")
}

/// Runs `loft interface add` with `args` in `scratch`, and asserts that it is refused, naming
/// `named`, with nothing written.
fn add_refused(scratch: &Scratch, args: &[&str], named: &str) {
    let before = tree(&scratch.0);
    let mut all = vec!["interface", "add"];
    all.extend(args);
    all.extend(["--format", "json"]);
    let (output, _) = run(scratch, &all, BrowserScenario::Absent, None);
    assert_named(&output, named);
    assert_eq!(tree(&scratch.0), before, "{args:?}: nothing is written");
}

#[test]
fn another_document_s_address_that_holds_a_backslash_is_named_with_it_doubled() {
    let remote = backslashed(OTHER, "Shelves", &shelves().states);
    let at = doubled(&remote.address());
    // A document the remote does not hold, at the address `--remote` gives.
    let scratch = listing_one(&remote, false);
    let missing = document_address(remote.origin(), "missing\\x");
    keep(
        &scratch,
        &json!({ missing.clone(): granted(&remote, 3600) }),
    );
    let named = format!(
        "{HANDLE}: the remote at {} holds no such document",
        doubled(&missing)
    );
    add_refused(&scratch, &["shelves:IF-1", "--remote", &missing], &named);
    // A publish sequence the remote does not hold.
    let scratch = listing_one(&remote, true);
    let named = format!("the remote at {at} holds no published state of publish sequence 7");
    add_refused(&scratch, &["shelves:IF-1", "--at", "7"], &named);
    // A document that `documents` lists under another handle already.
    let named = format!(
        "the remote at {at} serves the document {OTHER}, which `documents` lists under {HANDLE} \
         already"
    );
    add_refused(
        &scratch,
        &["stock:IF-2", "--remote", &remote.address()],
        &named,
    );
    // Another document than the one `documents` lists.
    let another = "00000000-0000-4000-8000-0000000004ff";
    scratch.edit(MANIFEST, OTHER, another);
    let named = format!("the remote at {at} serves the document {OTHER}, not {another}");
    add_refused(&scratch, &["shelves:IF-1"], &named);
    // A document that holds no published state.
    let empty = backslashed(OTHER, "Shelves", &[]);
    let named = format!(
        "{HANDLE}: the remote at {} holds no published state",
        doubled(&empty.address())
    );
    add_refused(&listing_one(&empty, true), &["shelves:IF-1"], &named);
    // The document itself.
    let own = history(1);
    let itself = backslashed(&own.key, "Reading List", &own.states);
    let named = format!(
        "the remote at {} serves the document {} itself",
        doubled(&itself.address()),
        own.key
    );
    add_refused(&listing_one(&itself, true), &["shelves:IF-1"], &named);
}
