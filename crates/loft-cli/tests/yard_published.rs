//! `loft yard published` as a process: on the base branch, the `publish` view recorded in the
//! Yard Log the merge brought in, in its last stage, after its close; refused on another branch,
//! for Yard Log the base branch does not hold, and a second time, changing no file.

use software_plant::GitScenario;

use crate::snapshot::snapshot;
use crate::yard::{ASK, BRANCH, HEAD, Yard, block, document};

/// A repository on `branch` whose HEAD is [`HEAD`], with the person named.
fn on(branch: &str) -> GitScenario {
    GitScenario::clean_on(branch)
        .base("HEAD", HEAD)
        .with_config("user.name", "Ada Reader")
        .with_config("user.email", "ada@example.com")
}

/// The Reading List's tree with the closed Yard Log of [`BRANCH`], now on `develop`.
fn merged() -> Yard {
    let mut yard = Yard::with_spec();
    yard.git = on(BRANCH);
    yard.write("s1", ASK, &[]);
    yard.run(&["close", "--session", "s1"]);
    yard.git = on("develop");
    yard
}

fn published(yard: &Yard, branch: &str) -> (Option<i32>, serde_json::Value) {
    let output = yard.run(&[
        "published",
        "--branch",
        branch,
        "--session",
        "s2",
        "--note",
        "Published.",
    ]);
    (output.status.code(), document("published", &output))
}

#[test]
fn a_publish_is_recorded_after_the_close_as_a_publish_view_holding_loft_s_report() {
    let mut yard = merged();
    let (code, recorded) = published(&yard, BRANCH);
    assert_eq!(code, Some(0), "{recorded}");
    assert_eq!(recorded["outcome"], "recorded");
    assert_eq!(recorded["branch"], BRANCH);
    assert_eq!(recorded["block"]["id"], "published");
    let status = yard.loft(&["status"]);
    yard.git = on(BRANCH);
    let read = yard.read("s1", &["--reports"]);
    let body = &block(&read, "published")["version"]["body"];
    assert_eq!(body["view"], "publish");
    assert_eq!(body["note"], "Published.");
    assert_eq!(body["recorded"]["branch"], "develop");
    assert_eq!(body["report"]["status"], status);
    let last = read["stages"].as_array().unwrap().last().unwrap();
    let blocks = last["blocks"].as_array().unwrap();
    assert_eq!(blocks.last().unwrap()["id"], "published");
}

#[test]
fn yard_log_reads_as_closed_as_before_with_the_publish_after_its_close() {
    let mut yard = merged();
    yard.git = on(BRANCH);
    let before = yard.read("s1", &[]);
    yard.git = on("develop");
    published(&yard, BRANCH);
    yard.git = on(BRANCH);
    let after = yard.read("s1", &[]);
    assert_eq!(after["yardLog"]["closed"], before["yardLog"]["closed"]);
    assert_eq!(after["yardLog"]["closedBy"], "agent");
    let at = |read: &serde_json::Value| read["yardLog"]["closed"]["revision"].as_u64().unwrap();
    let publish = block(&after, "published")["version"]["act"]["revision"]
        .as_u64()
        .unwrap();
    assert!(publish > at(&after));
}

#[test]
fn a_publish_on_a_branch_other_than_the_base_branch_is_refused_naming_the_three_branches() {
    let mut yard = merged();
    yard.git = on("feat/other");
    let before = snapshot(&yard.scratch.0);
    let (code, refused) = published(&yard, BRANCH);
    assert_eq!(code, Some(1));
    assert_eq!(refused["errors"][0]["kind"], "not-base");
    let message = refused["errors"][0]["message"].as_str().unwrap();
    for name in ["feat/other", "develop", BRANCH] {
        assert!(message.contains(name), "{message}");
    }
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

#[test]
fn a_publish_for_yard_log_the_base_branch_does_not_hold_is_refused() {
    let yard = merged();
    let before = snapshot(&yard.scratch.0);
    let (code, refused) = published(&yard, "feat/none");
    assert_eq!(code, Some(1));
    assert_eq!(refused["errors"][0]["kind"], "absent");
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("feat/none"), "{message}");
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

#[test]
fn a_second_publish_after_the_close_is_refused_and_changes_no_file() {
    let yard = merged();
    published(&yard, BRANCH);
    let before = snapshot(&yard.scratch.0);
    let (code, refused) = published(&yard, BRANCH);
    assert_eq!(code, Some(1));
    assert_eq!(refused["errors"][0]["kind"], "recorded");
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}
