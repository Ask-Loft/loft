//! `loft verify` as a process (F-42, F-31): the roots recomputed from the tree and compared with
//! the lock, with an exit code a CI job can gate on.

mod common;
#[path = "common/schema.rs"]
mod schema;

use std::fs;
use std::process::Output;

use common::{EPOCH, Scratch, stderr, stdout};
use loftspec::publish::{LOCK, Lock};
use serde_json::Value;
use software_plant::{Clock, GitScenario};

/// The Reading List, published.
fn published() -> Scratch {
    let scratch = Scratch::with_reading_list();
    assert_eq!(
        scratch
            .loft_with(
                &["publish"],
                Some(&GitScenario::clean_on("main")),
                &Clock::fixed(EPOCH)
            )
            .status
            .code(),
        Some(0)
    );
    scratch
}

fn state_root(scratch: &Scratch) -> String {
    let bytes = fs::read(scratch.0.join(LOCK)).unwrap();
    Lock::read(&bytes).unwrap().state_root.to_string()
}

fn json(output: &Output) -> Value {
    let document: Value = serde_json::from_str(&stdout(output)).unwrap();
    let schema: Value = serde_json::from_str(include_str!("../../../schemas/verify.json")).unwrap();
    assert_eq!(schema::violations(&document, &schema), Vec::<String>::new());
    document
}

#[test]
fn ac_560_on_a_match_it_prints_the_state_root_and_exits_0() {
    let scratch = published();
    let output = scratch.loft(&["verify"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!("verified: state root {}\n", state_root(&scratch))
    );
}

#[test]
fn ac_561_on_a_mismatch_it_names_the_first_differing_root_and_exits_1() {
    let scratch = published();
    scratch.edit(
        "products/reading-list/product.yaml",
        "Keeps the books a reader means to read.",
        "Keeps the books a reader has read.",
    );
    let output = scratch.loft(&["verify"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).starts_with("the content root differs: loft.lock.json has "));
}

#[test]
fn ac_562_without_a_lock_it_exits_1_naming_the_missing_file() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["verify"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("loft.lock.json: there is no lock"));
}

#[test]
fn ac_667_a_lock_that_is_not_a_valid_lock_record_exits_1_naming_the_lock_file() {
    let scratch = published();
    let text = fs::read_to_string(scratch.0.join(LOCK)).unwrap();
    fs::write(
        scratch.0.join(LOCK),
        text.replace("\"publishSequence\": 1", "\"publishSequence\": 0"),
    )
    .unwrap();
    let output = scratch.loft(&["verify"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("loft.lock.json: not a valid lock record"));
}

#[test]
fn ac_564_a_read_error_names_no_root() {
    let scratch = published();
    scratch.edit(
        "products/reading-list/product.yaml",
        "\nslug:",
        " # a note\nslug:",
    );
    let output = scratch.loft(&["verify"]);
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output) + &stderr(&output);
    assert!(!text.contains("root"), "{text}");
}

#[test]
fn ac_470_verification_writes_nothing() {
    let scratch = published();
    let before = fs::read(scratch.0.join(LOCK)).unwrap();
    scratch.loft(&["verify"]);
    assert_eq!(fs::read(scratch.0.join(LOCK)).unwrap(), before);
}

#[test]
fn ac_582_the_json_results_match_the_verify_schema() {
    let scratch = published();
    let verified = json(&scratch.loft(&["verify", "--format", "json"]));
    assert_eq!(verified["outcome"], "verified");
    assert_eq!(verified["stateRoot"], state_root(&scratch).as_str());
    scratch.edit(
        "products/reading-list/product.yaml",
        "Keeps the books a reader means to read.",
        "Keeps the books a reader has read.",
    );
    let mismatch = json(&scratch.loft(&["verify", "--format", "json"]));
    assert_eq!(mismatch["outcome"], "mismatch");
    assert_eq!(mismatch["root"], "contentRoot");
    fs::remove_file(scratch.0.join(LOCK)).unwrap();
    let missing = json(&scratch.loft(&["verify", "--format", "json"]));
    assert_eq!(missing["outcome"], "error");
    assert_eq!(missing["errors"][0]["kind"], "lock");
}
