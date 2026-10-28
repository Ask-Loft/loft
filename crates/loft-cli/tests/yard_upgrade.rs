//! `loft yard upgrade` as a process, over every branch's Yard Log under the tree's `.yard/`:
//! closed Yard Log and Yard Log already in this `loft`'s format left as they are, one of a
//! newer format or one that does not read left and named with exit 1, and a second run that
//! changes nothing.

use std::fs;
use std::process::Output;

use serde_json::Value;

use crate::common::workspace;
use crate::schema;
use crate::snapshot::snapshot;
use crate::yard::{ASK, BRANCH, Yard, document};

/// Runs `loft yard upgrade`, giving its exit code and its JSON document, which conforms to the
/// command's schema.
pub(crate) fn upgrade(yard: &Yard) -> (Option<i32>, Value) {
    conforming(&yard.run(&["upgrade"]))
}

pub(crate) fn conforming(output: &Output) -> (Option<i32>, Value) {
    let doc = document("upgrade", output);
    let schema = fs::read_to_string(workspace().join("schemas/yard-upgrade.json")).unwrap();
    let text = serde_json::to_vec(&doc).unwrap();
    (output.status.code(), schema::conforming(&text, &schema))
}

#[test]
fn yard_log_already_in_the_current_format_is_left_as_it_was_and_a_second_run_changes_nothing() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let before = snapshot(&yard.scratch.0.join(".yard"));
    let (code, first) = upgrade(&yard);
    assert_eq!(code, Some(0));
    assert_eq!(first["current"][0]["branch"], BRANCH);
    assert_eq!(first["upgraded"], serde_json::json!([]));
    let (_, second) = upgrade(&yard);
    assert_eq!(second, first);
    assert!(
        snapshot(&yard.scratch.0.join(".yard")) == before,
        "a file changed"
    );
}

#[test]
fn closed_yard_log_is_left_as_it_was_and_listed_as_left() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.run(&["close", "--session", "s1"]);
    let before = snapshot(&yard.scratch.0.join(".yard"));
    let (code, result) = upgrade(&yard);
    assert_eq!(code, Some(0));
    assert_eq!(result["left"][0]["branch"], BRANCH);
    assert_eq!(result["left"][0]["reason"], "closed");
    assert!(
        snapshot(&yard.scratch.0.join(".yard")) == before,
        "a file changed"
    );
}

#[test]
fn yard_log_of_a_newer_format_or_that_does_not_read_is_left_named_and_exits_1() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let newer = yard.folder("feat/later");
    fs::create_dir_all(&newer).unwrap();
    fs::write(newer.join("yardlog.yaml"), "format: yardlog/9\n").unwrap();
    let broken = yard.folder("feat/broken");
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("yardlog.yaml"), "format: yardlog/1\nkey: [\n").unwrap();
    let before = snapshot(&yard.scratch.0.join(".yard"));
    let (code, result) = upgrade(&yard);
    assert_eq!(code, Some(1));
    assert_eq!(result["outcome"], "left some");
    let left: Vec<&str> = result["left"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["branch"].as_str().unwrap())
        .collect();
    assert_eq!(left, ["feat/broken", "feat/later"]);
    assert!(
        result["left"][1]["reason"]
            .as_str()
            .unwrap()
            .contains("yardlog/9")
    );
    assert_eq!(result["current"][0]["branch"], BRANCH);
    assert_eq!(result["errors"].as_array().unwrap().len(), 2);
    assert!(
        snapshot(&yard.scratch.0.join(".yard")) == before,
        "a file changed"
    );
}
