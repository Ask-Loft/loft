//! `loft publish` as a process (F-41, F-27 to F-30), run in front of the Software Plant's git
//! simulator and clock: what it prints, what it writes, and when it refuses.

mod common;
#[path = "common/schema.rs"]
mod schema;

use std::fs;
use std::process::{Command, Output};

use common::{EPOCH, Scratch, stderr, stdout};
use loftspec::codec::Files;
use loftspec_store::FileStore;
use serde_json::Value;
use software_plant::{Clock, GitScenario};

const LOCK: &str = "loft.lock.json";

fn lock(scratch: &Scratch) -> Option<String> {
    fs::read_to_string(scratch.0.join(LOCK)).ok()
}

/// The files of the Format under the scratch directory: the tree and the lock.
fn format_files(scratch: &Scratch) -> (Files, Option<String>) {
    let files = FileStore::open(&scratch.0).unwrap().read_files().unwrap();
    (files, lock(scratch))
}

fn json(output: &Output) -> Value {
    let document: Value = serde_json::from_str(&stdout(output)).unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/publish.json")).unwrap();
    assert_eq!(schema::violations(&document, &schema), Vec::<String>::new());
    document
}

/// Leaves the binding of `label` out of `spec/labels.yaml`, so the next publish allocates.
fn unlabel(scratch: &Scratch, label: &str) -> String {
    let text = fs::read_to_string(scratch.file("labels.yaml")).unwrap();
    let line = format!("    label: {label}\n");
    let at = text.find(&line).unwrap();
    let start = text[..at].rfind("  - key: ").unwrap();
    let key = text[start + 9..at].trim().to_owned();
    let end = at + line.len();
    fs::write(
        scratch.file("labels.yaml"),
        format!("{}{}", &text[..start], &text[end..]),
    )
    .unwrap();
    key
}

/// Runs `loft` with a search path that holds no `git`, and the given environment.
fn loft_without_git(scratch: &Scratch, args: &[&str], env: &[(&str, &str)]) -> Output {
    let empty = scratch.0.join("out-of-path");
    fs::create_dir_all(&empty).unwrap();
    Command::new(env!("CARGO_BIN_EXE_loft"))
        .args(args)
        .env("PATH", &empty)
        .envs(env.iter().copied())
        .current_dir(&scratch.0)
        .output()
        .unwrap()
}

#[test]
fn ac_554_a_first_publish_writes_the_lock_and_prints_sequence_1_and_the_state_root() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["publish"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let lock = lock(&scratch).unwrap();
    let state = lock
        .lines()
        .find_map(|line| line.trim().strip_prefix("\"stateRoot\": \""))
        .unwrap()
        .trim_end_matches("\",");
    assert_eq!(
        stdout(&output),
        format!("published sequence 1, state root {state}\n")
    );
}

#[test]
fn the_publish_time_is_the_one_the_clock_gives_in_utc() {
    let scratch = Scratch::with_reading_list();
    assert_eq!(scratch.loft(&["publish"]).status.code(), Some(0));
    assert!(
        lock(&scratch)
            .unwrap()
            .contains("\"publishedAt\": \"2026-01-01T00:00:00Z\"")
    );
    assert_eq!(EPOCH, 1_767_225_600);
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft_with(
        &["publish"],
        &GitScenario::clean_on("main"),
        &Clock::fixed(0),
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(
        lock(&scratch)
            .unwrap()
            .contains("\"publishedAt\": \"1970-01-01T00:00:00Z\"")
    );
}

#[test]
fn ac_553_a_publish_prints_each_allocated_label_with_its_key() {
    let scratch = Scratch::with_reading_list();
    let key = unlabel(&scratch, "AC-3");
    let output = scratch.loft(&["publish"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let first = stdout(&output).lines().next().unwrap().to_owned();
    assert_eq!(first, format!("AC-8 {key}"));
    assert!(
        fs::read_to_string(scratch.file("labels.yaml"))
            .unwrap()
            .contains("label: AC-8")
    );
}

#[test]
fn ac_555_ac_464_a_second_publish_with_nothing_changed_writes_nothing_and_exits_0() {
    let scratch = Scratch::with_reading_list();
    scratch.loft(&["publish"]);
    let before = format_files(&scratch);
    let output = scratch.loft_with(
        &["publish"],
        &GitScenario::clean_on("main"),
        &Clock::fixed(EPOCH + 60),
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).starts_with("nothing changed since publish sequence 1, state root "));
    assert!(format_files(&scratch) == before, "a file changed");
}

/// Runs a publish that must refuse, and checks that it wrote nothing and exited 1 with
/// `reason` in what it printed.
fn refuses(scratch: &Scratch, output: &Output, reason: &str) {
    assert_eq!(output.status.code(), Some(1), "{}", stdout(output));
    let text = stderr(output);
    assert!(text.contains(reason), "{reason:?} in {text}");
    assert_eq!(lock(scratch), None, "a lock was written");
}

#[test]
fn ac_447_ac_552_uncommitted_changes_under_spec_refuse_naming_the_paths() {
    let scratch = Scratch::with_reading_list();
    let git = GitScenario::clean_on("main")
        .with(" M", "spec/tags.yaml")
        .with("??", "spec/products/new.yaml");
    let output = scratch.loft_with(&["publish"], &git, &Clock::fixed(EPOCH));
    refuses(&scratch, &output, "spec/products/new.yaml, spec/tags.yaml");
}

#[test]
fn ac_450_another_branch_refuses_naming_both_branches() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft_with(
        &["publish"],
        &GitScenario::clean_on("develop"),
        &Clock::fixed(EPOCH),
    );
    refuses(
        &scratch,
        &output,
        "on branch `develop`, but the publish branch is `main`",
    );
}

#[test]
fn ac_690_a_detached_head_refuses() {
    let scratch = Scratch::with_reading_list();
    let git = GitScenario::clean_on("main").detached();
    let output = scratch.loft_with(&["publish"], &git, &Clock::fixed(EPOCH));
    refuses(
        &scratch,
        &output,
        "HEAD is detached, not on the publish branch `main`",
    );
}

#[test]
fn ac_558_ac_559_outside_a_git_repository_a_publish_refuses_and_writes_nothing() {
    let scratch = Scratch::with_reading_list();
    let before = format_files(&scratch);
    let output = scratch.loft_with(
        &["publish"],
        &GitScenario::outside_repository(),
        &Clock::fixed(EPOCH),
    );
    refuses(&scratch, &output, "git repository");
    assert!(format_files(&scratch) == before, "a file changed");
}

#[test]
fn a_git_that_cannot_be_run_refuses_naming_git() {
    let scratch = Scratch::with_reading_list();
    let output = loft_without_git(&scratch, &["publish"], &[]);
    refuses(&scratch, &output, "git cannot be run");
}

#[test]
fn a_failing_git_refuses_with_its_message() {
    let scratch = Scratch::with_reading_list();
    let git = GitScenario::clean_on("main").failing(
        software_plant::git::Call::Status,
        128,
        "fatal: index file corrupt\n",
    );
    let output = scratch.loft_with(&["publish"], &git, &Clock::fixed(EPOCH));
    refuses(&scratch, &output, "fatal: index file corrupt");
}

#[test]
fn an_invalid_lock_refuses_naming_the_lock_file() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.0.join(LOCK), "{}\n").unwrap();
    let output = scratch.loft(&["publish"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("loft.lock.json: not a valid lock record"));
    assert_eq!(lock(&scratch).as_deref(), Some("{}\n"));
}

#[test]
fn an_epoch_that_is_not_a_whole_number_of_seconds_refuses_naming_it() {
    let scratch = Scratch::with_reading_list();
    let mut command = Command::new(env!("CARGO_BIN_EXE_loft"));
    let plant = scratch.0.join("plant");
    fs::create_dir_all(&plant).unwrap();
    GitScenario::clean_on("main")
        .install(&plant)
        .apply(&mut command);
    let output = command
        .arg("publish")
        .env("SOURCE_DATE_EPOCH", "soon")
        .current_dir(&scratch.0)
        .output()
        .unwrap();
    refuses(&scratch, &output, "SOURCE_DATE_EPOCH");
}

#[test]
fn ac_451_a_validation_error_refuses_listing_the_errors() {
    let scratch = Scratch::with_reading_list();
    let decisions = scratch.0.join("spec/decisions");
    let accepted = fs::read_dir(&decisions)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            let text = fs::read_to_string(path).unwrap();
            text.contains("  kind: accepted\n") && text.contains("\nsubjects:\n")
        })
        .expect("an accepted decision with subjects");
    let mut text = fs::read_to_string(&accepted).unwrap();
    text.push_str("  - role: affected\n    node:\n      formerLabel: AC-99\n");
    fs::write(&accepted, text).unwrap();
    let output = scratch.loft(&["publish"]);
    refuses(&scratch, &output, "AC-99");
    assert!(stderr(&output).contains("the validator reports"));
}

#[test]
fn a_read_error_is_reported_before_any_other_refusal() {
    let scratch = Scratch::with_reading_list();
    scratch.edit(
        "products/reading-list/product.yaml",
        "\nslug:",
        " # a note\nslug:",
    );
    let output = scratch.loft_with(
        &["publish"],
        &GitScenario::outside_repository(),
        &Clock::fixed(EPOCH),
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).starts_with("spec/products/reading-list/product.yaml:1:"));
}

#[test]
fn the_json_results_match_the_publish_schema() {
    let scratch = Scratch::with_reading_list();
    let key = unlabel(&scratch, "AC-3");
    let published = json(&scratch.loft(&["publish", "--format", "json"]));
    assert_eq!(published["outcome"], "published");
    assert_eq!(published["publishSequence"], 1);
    assert_eq!(published["allocated"][0]["label"], "AC-8");
    assert_eq!(published["allocated"][0]["key"], key.as_str());
    let unchanged = json(&scratch.loft(&["publish", "--format", "json"]));
    assert_eq!(unchanged["outcome"], "unchanged");
    let refused = json(&scratch.loft_with(
        &["publish", "--format", "json"],
        &GitScenario::clean_on("develop"),
        &Clock::fixed(EPOCH),
    ));
    assert_eq!(refused["outcome"], "refused");
    assert_eq!(refused["errors"][0]["kind"], "refused");
}
