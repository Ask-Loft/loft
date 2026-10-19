//! `loft check --baseline` as a process (the `loft check` Feature): the findings marked
//! introduced or preexisting against the base revision's tree read from git, the gate on the
//! introduced ones, the record of where the comparison came from, and the base it refuses.

use std::fs;
use std::process::Output;

use crate::base::{COMMIT, based_on_now};
use crate::common::{EPOCH, Scratch, stderr, stdout};
use crate::schema;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use software_plant::{Clock, GitScenario};

const FEATURE: &str = "products/reading-list/shelf/remove-a-book.yaml";
const PLAIN: &str = "\"A removed book no longer lists.\"";
const VAGUE: &str = "\"A removed book no longer lists in some views.\"";
const PRODUCT: &str = "products/reading-list/product.yaml";
/// The six info findings the Reading List holds.
const HELD: usize = 6;

fn check(scratch: &Scratch, args: &[&str], git: &GitScenario) -> Output {
    let mut all = vec!["check", "--baseline", "develop"];
    all.extend_from_slice(args);
    scratch.loft_with(&all, Some(git), &Clock::fixed(EPOCH))
}

fn json(output: &Output) -> Value {
    schema::conforming(&output.stdout, include_str!("../../../schemas/check.json"))
}

/// The Reading List, its base the tree as it is, then a vague quantifier added to a criterion.
fn introducing() -> (Scratch, GitScenario) {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    scratch.edit(FEATURE, PLAIN, VAGUE);
    (scratch, git)
}

/// The changes the findings of `document` carry, by rule.
fn changes(document: &Value) -> Vec<(String, String)> {
    document["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            let rule = finding["rule"].as_str().unwrap().to_owned();
            (rule, finding["change"].as_str().unwrap().to_owned())
        })
        .collect()
}

fn summary(introduced: usize, preexisting: usize, resolved: usize) -> String {
    format!(
        "baseline: develop at {COMMIT}: {introduced} introduced, {preexisting} preexisting, \
         {resolved} resolved\n"
    )
}

#[test]
fn an_unchanged_tree_prints_no_finding_and_counts_each_one_preexisting() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    let output = check(&scratch, &[], &git);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout(&output), summary(0, HELD, 0));
}

#[test]
fn an_introduced_finding_is_printed_and_preexisting_ones_are_not() {
    let (scratch, git) = introducing();
    let text = stdout(&check(&scratch, &[], &git));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(
        lines[0].starts_with(&format!("spec/{FEATURE}:")) && lines[0].contains("LQ-002"),
        "{text}"
    );
    assert_eq!(format!("{}\n", lines[1]), summary(1, HELD, 0));
}

#[test]
fn each_finding_carries_its_change_and_the_base_is_named() {
    let (scratch, git) = introducing();
    let document = json(&check(&scratch, &["--format", "json"], &git));
    let introduced: Vec<_> = changes(&document)
        .into_iter()
        .filter(|(_, change)| change == "introduced")
        .collect();
    assert_eq!(introduced, [("LQ-002".to_owned(), "introduced".to_owned())]);
    assert_eq!(changes(&document).len(), HELD + 1);
    assert_eq!(document["baseline"]["revision"], "develop");
    assert_eq!(document["baseline"]["commit"], COMMIT);
    assert_eq!(document["baseline"]["loft"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn the_base_tree_is_read_from_git_not_from_the_working_tree() {
    let scratch = Scratch::with_reading_list();
    scratch.edit(FEATURE, PLAIN, VAGUE);
    let git = based_on_now(&scratch);
    scratch.edit(FEATURE, VAGUE, PLAIN);
    let document = json(&check(&scratch, &["--format", "json"], &git));
    assert!(
        changes(&document)
            .iter()
            .all(|(_, change)| change == "preexisting")
    );
    let resolved = document["resolved"].as_array().unwrap();
    assert_eq!(resolved.len(), 1, "{document}");
    assert_eq!(resolved[0]["rule"], "LQ-002");
    assert_eq!(resolved[0]["path"], format!("spec/{FEATURE}"));
    assert_eq!(resolved[0].get("change"), None);
    let text = stdout(&check(&scratch, &[], &git));
    assert_eq!(text, summary(0, HELD, 1));
}

#[test]
fn fail_on_counts_introduced_findings_alone() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    let without = scratch.loft(&["check", "--fail-on", "hint"]);
    assert_eq!(without.status.code(), Some(1));
    let output = check(&scratch, &["--fail-on", "hint", "--format", "json"], &git);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(json(&output)["outcome"], "passed");
    let (scratch, git) = introducing();
    let output = check(
        &scratch,
        &["--fail-on", "warning", "--format", "json"],
        &git,
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(json(&output)["outcome"], "failed");
}

#[test]
fn a_working_tree_error_still_exits_1_against_a_base() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    scratch.edit(PRODUCT, "  - portable\n", "  - portable\n  - portable\n");
    let output = check(&scratch, &["--format", "json"], &git);
    assert_eq!(output.status.code(), Some(1));
    let document = json(&output);
    assert_eq!(document["outcome"], "failed");
    assert_eq!(document["errors"][0]["kind"], "validation");
}

fn digest(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

#[test]
fn the_lint_configurations_are_named_by_digest_and_compared() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    let document = json(&check(&scratch, &["--format", "json"], &git));
    let config = &document["baseline"]["lintConfig"];
    assert_eq!(config["tree"], Value::Null);
    assert_eq!(config["base"], Value::Null);
    assert_eq!(config["same"], true);
    let quiet = "rules:\n  - rule: SH-002\n    enabled: false\n";
    fs::write(scratch.file("lint.yaml"), quiet).unwrap();
    let document = json(&check(&scratch, &["--format", "json"], &git));
    let config = &document["baseline"]["lintConfig"];
    assert_eq!(config["tree"], digest(quiet));
    assert_eq!(config["base"], Value::Null);
    assert_eq!(config["same"], false);
}

#[test]
fn without_baseline_the_document_carries_no_change_resolved_or_baseline() {
    let scratch = Scratch::with_reading_list();
    let document = json(&scratch.loft(&["check", "--format", "json"]));
    assert_eq!(document.get("resolved"), None);
    assert_eq!(document.get("baseline"), None);
    for finding in document["findings"].as_array().unwrap() {
        assert_eq!(finding.get("change"), None, "{finding}");
    }
}

/// `loft check --baseline <revision>` refused under `git`: its standard error, and its JSON
/// document's errors.
fn refused(scratch: &Scratch, revision: &str, git: &GitScenario) -> (String, Value) {
    let run = |extra: &[&str]| {
        let mut all = vec!["check", "--baseline", revision];
        all.extend_from_slice(extra);
        scratch.loft_with(&all, Some(git), &Clock::fixed(EPOCH))
    };
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    assert_eq!(stdout(&output), "");
    let document = json(&run(&["--format", "json"]));
    assert_eq!(document["outcome"], "error");
    assert_eq!(document.get("findings"), None);
    assert_eq!(document["errors"][0]["kind"], "base");
    (stderr(&output), document)
}

#[test]
fn outside_a_repository_a_baseline_is_refused() {
    let scratch = Scratch::with_reading_list();
    let (error, _) = refused(&scratch, "develop", &GitScenario::outside_repository());
    assert!(error.contains("not inside a git repository"), "{error}");
}

#[test]
fn a_baseline_that_names_no_commit_is_refused_naming_it() {
    let scratch = Scratch::with_reading_list();
    let (error, _) = refused(&scratch, "nosuch", &GitScenario::clean_on("feature"));
    assert_eq!(error, "`nosuch` names no commit\n");
}

#[test]
fn a_baseline_without_a_common_ancestor_is_refused_and_says_so() {
    let scratch = Scratch::with_reading_list();
    let git = GitScenario::clean_on("feature").unrelated("orphan");
    let (error, _) = refused(&scratch, "orphan", &git);
    assert_eq!(error, "`orphan` shares no history with HEAD\n");
}

#[test]
fn a_base_tree_that_does_not_read_is_refused_naming_the_commit_and_the_place() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch).file_at(
        "develop",
        "spec/products/reading-list/shelf/broken.yaml",
        "ffff000000000000000000000000000000000000",
        "name: [broken\n",
    );
    let (error, _) = refused(&scratch, "develop", &git);
    assert!(
        error.starts_with(&format!(
            "the tree at {COMMIT} does not read: spec/products/reading-list/shelf/broken.yaml:1:"
        )),
        "{error}"
    );
}
