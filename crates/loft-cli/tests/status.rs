//! `loft status` as a process (the `loft status` Feature): the lock and whether the tree matches
//! it, the repository's state, and, against a base revision, the nodes added, changed and
//! removed since HEAD left it, over the Reading List.

use std::fs;
use std::path::Path;
use std::process::Output;

use crate::common::{EPOCH, Scratch, stderr, stdout};
use crate::schema;
use crate::snapshot::snapshot;
use serde_json::Value;
use software_plant::{Clock, GitScenario};

const COMMIT: &str = "c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00";
const FEATURE: &str = "products/reading-list/shelf/remove-a-book.yaml";
/// The criterion AC-4 of the Feature "Remove a book", under RQ-3.
const ABSENT: &str = "00000000-0000-4000-8000-00000000002f";
/// The criterion AC-5 of the Feature "Remove a book", under RQ-4.
const GONE: &str = "00000000-0000-4000-8000-000000000031";
const ADDED: &str = "00000000-0000-4000-8000-0000000000aa";

fn json(output: &Output) -> Value {
    schema::conforming(&output.stdout, include_str!("../../../schemas/status.json"))
}

/// Every file under `dir`, by its path from `dir`.
fn files(dir: &Path, prefix: &str, into: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let relative = format!("{prefix}{name}");
        if path.is_dir() {
            files(&path, &format!("{relative}/"), into);
        } else {
            into.push((relative, fs::read_to_string(&path).unwrap()));
        }
    }
}

/// A clean repository on `feature` whose base for `develop` holds the tree as it now is.
fn based_on_now(scratch: &Scratch) -> GitScenario {
    let mut held = Vec::new();
    files(&scratch.0.join("spec"), "spec/", &mut held);
    let mut scenario = GitScenario::clean_on("feature").base("develop", COMMIT);
    for (number, (path, text)) in held.iter().enumerate() {
        scenario = scenario.file_at("develop", path, &format!("{number:040x}"), text);
    }
    scenario
}

fn status(scratch: &Scratch, args: &[&str], git: &GitScenario) -> Output {
    let mut all = vec!["status"];
    all.extend_from_slice(args);
    scratch.loft_with(&all, Some(git), &Clock::fixed(EPOCH))
}

/// The keys of one of the base's lists.
fn keys(document: &Value, list: &str) -> Vec<String> {
    document["base"][list]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["key"].as_str().unwrap().to_owned())
        .collect()
}

fn published() -> Scratch {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft_with(
        &["publish"],
        Some(&GitScenario::clean_on("main")),
        &Clock::fixed(EPOCH),
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    scratch
}

#[test]
fn the_report_gives_the_locks_publish_sequence_publication_time_and_state_root() {
    let scratch = published();
    let output = status(
        &scratch,
        &["--format", "json"],
        &GitScenario::clean_on("main"),
    );
    let document = json(&output);
    let lock = &document["lock"];
    assert_eq!(lock["publishSequence"], 1);
    assert_eq!(lock["publishedAt"], "2026-01-01T00:00:00Z");
    assert_eq!(lock["stateRoot"].as_str().unwrap().len(), 64);
}

#[test]
fn the_report_says_that_the_tree_matches_the_lock_when_every_root_matches() {
    let scratch = published();
    let output = status(&scratch, &[], &GitScenario::clean_on("main"));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output)
            .lines()
            .next()
            .unwrap()
            .ends_with("; the tree matches it")
    );
    let document = json(&status(
        &scratch,
        &["--format", "json"],
        &GitScenario::clean_on("main"),
    ));
    assert_eq!(document["lock"]["matches"], true);
    assert_eq!(document["lock"]["differs"], Value::Null);
}

#[test]
fn the_report_names_the_first_root_that_differs_from_the_lock() {
    let scratch = published();
    scratch.edit(
        "products/reading-list/product.yaml",
        "Keeps the books a reader means to read.",
        "Keeps the books a reader has read.",
    );
    let output = status(&scratch, &[], &GitScenario::clean_on("main"));
    assert_eq!(output.status.code(), Some(0));
    assert!(
        stdout(&output)
            .lines()
            .next()
            .unwrap()
            .ends_with("; the tree's content root differs")
    );
    let document = json(&status(
        &scratch,
        &["--format", "json"],
        &GitScenario::clean_on("main"),
    ));
    assert_eq!(document["lock"]["matches"], false);
    assert_eq!(document["lock"]["differs"], "contentRoot");
}

#[test]
fn without_a_lock_the_report_says_the_tree_has_not_been_published_and_exits_0() {
    let scratch = Scratch::with_reading_list();
    let output = status(&scratch, &[], &GitScenario::clean_on("main"));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).starts_with("lock: none; the tree has not been published"));
    let document = json(&status(
        &scratch,
        &["--format", "json"],
        &GitScenario::clean_on("main"),
    ));
    assert_eq!(document["lock"], Value::Null);
}

#[test]
fn no_file_is_written() {
    let scratch = published();
    let before = snapshot(&scratch.0);
    let git = based_on_now(&scratch);
    assert_eq!(
        status(&scratch, &["--base", "develop"], &git).status.code(),
        Some(0)
    );
    assert_eq!(snapshot(&scratch.0), before);
}

#[test]
fn the_report_names_the_current_branch_or_says_that_head_is_detached() {
    let scratch = Scratch::with_reading_list();
    let on = json(&status(
        &scratch,
        &["--format", "json"],
        &GitScenario::clean_on("feature"),
    ));
    assert_eq!(on["repository"]["branch"], "feature");
    let detached = GitScenario::clean_on("main").detached();
    let output = status(&scratch, &[], &detached);
    assert!(stdout(&output).contains("branch: none; HEAD is detached\n"));
    let document = json(&status(&scratch, &["--format", "json"], &detached));
    assert_eq!(document["repository"]["branch"], Value::Null);
}

#[test]
fn the_report_lists_each_path_under_spec_that_has_uncommitted_changes() {
    let scratch = Scratch::with_reading_list();
    let git = GitScenario::clean_on("main")
        .with(" M", "spec/tags.yaml")
        .with("??", "spec/products/reading-list/shelf/new.yaml");
    let output = status(&scratch, &[], &git);
    assert!(stdout(&output).contains("uncommitted: spec/tags.yaml\n"));
    let document = json(&status(&scratch, &["--format", "json"], &git));
    assert_eq!(
        document["repository"]["modified"],
        serde_json::json!([
            "spec/products/reading-list/shelf/new.yaml",
            "spec/tags.yaml"
        ])
    );
}

#[test]
fn the_report_names_a_merge_or_a_rebase_that_stopped_on_a_conflict() {
    let scratch = Scratch::with_reading_list();
    let head = "1111111111111111111111111111111111111111";
    for (git, word) in [
        (GitScenario::clean_on("main").merging(head), "merge"),
        (
            GitScenario::clean_on("main").rebasing(head, "refs/heads/main"),
            "rebase",
        ),
    ] {
        let output = status(&scratch, &[], &git);
        assert!(
            stdout(&output).contains(&format!("stopped: a {word}\n")),
            "{}",
            stdout(&output)
        );
        let document = json(&status(&scratch, &["--format", "json"], &git));
        assert_eq!(document["repository"]["stopped"], word);
    }
}

#[test]
fn the_report_names_the_base_revision_and_the_commit_merge_base_gives() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    let output = status(&scratch, &["--base", "develop"], &git);
    assert!(
        stdout(&output).contains(&format!(
            "base: develop at {COMMIT}: 0 added, 0 changed, 0 removed\n"
        )),
        "{}",
        stdout(&output)
    );
    let document = json(&status(
        &scratch,
        &["--base", "develop", "--format", "json"],
        &git,
    ));
    assert_eq!(document["base"]["revision"], "develop");
    assert_eq!(document["base"]["commit"], COMMIT);
}

#[test]
fn a_node_only_the_tree_holds_is_added_and_carries_a_null_label() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    scratch.edit(
        FEATURE,
        "        name: \"A removed book no longer lists.\"\n",
        &format!(
            "        name: \"A removed book no longer lists.\"\n      - key: {ADDED}\n        name: \"A removed book leaves no gap.\"\n"
        ),
    );
    let document = json(&status(
        &scratch,
        &["--base", "develop", "--format", "json"],
        &git,
    ));
    assert_eq!(keys(&document, "added"), [ADDED]);
    let added = &document["base"]["added"][0];
    assert_eq!(added["label"], Value::Null);
    assert_eq!(added["kind"], "criterion");
    assert_eq!(added["name"], "A removed book leaves no gap.");
    assert_eq!(added["path"], format!("spec/{FEATURE}"));
    assert!(added["line"].as_u64().unwrap() > 1 && added["column"].as_u64().unwrap() > 1);
    let text = stdout(&status(&scratch, &["--base", "develop"], &git));
    assert!(
        text.contains(&format!("added  {ADDED}  -  criterion  spec/{FEATURE}:")),
        "{text}"
    );
}

#[test]
fn a_node_only_the_base_holds_is_removed_with_the_bases_label_and_site() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    let base_line = fs::read_to_string(scratch.file(FEATURE))
        .unwrap()
        .lines()
        .position(|line| line.contains(GONE))
        .unwrap()
        + 1;
    scratch.edit(
        FEATURE,
        &format!("    criteria:\n      - key: {GONE}\n        name: \"A removed book no longer lists.\"\n"),
        "    criteria:\n      - key: 00000000-0000-4000-8000-0000000000ab\n        name: \"A removed book is gone.\"\n",
    );
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0));
    let document = json(&status(
        &scratch,
        &["--base", "develop", "--format", "json"],
        &git,
    ));
    assert_eq!(keys(&document, "removed"), [GONE]);
    let removed = &document["base"]["removed"][0];
    assert_eq!(removed["label"], "AC-5");
    assert_eq!(removed["path"], format!("spec/{FEATURE}"));
    assert_eq!(
        usize::try_from(removed["line"].as_u64().unwrap()).unwrap(),
        base_line
    );
}

#[test]
fn a_node_whose_own_content_differs_is_changed_and_its_ancestors_are_not() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    scratch.edit(
        FEATURE,
        "Removing an absent book reports the book as absent and changes nothing.",
        "Removing an absent book reports it as absent.",
    );
    let document = json(&status(
        &scratch,
        &["--base", "develop", "--format", "json"],
        &git,
    ));
    assert_eq!(keys(&document, "changed"), [ABSENT]);
    assert_eq!(document["base"]["changed"][0]["label"], "AC-4");
    assert!(keys(&document, "added").is_empty() && keys(&document, "removed").is_empty());
}

#[test]
fn a_node_that_only_moved_to_another_parent_is_not_changed() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    let gone = format!("      - key: {GONE}\n        name: \"A removed book no longer lists.\"\n");
    scratch.edit(FEATURE, &gone, "");
    scratch.edit(
        FEATURE,
        "        name: \"Removing an absent book reports the book as absent and changes nothing.\"\n",
        &format!("        name: \"Removing an absent book reports the book as absent and changes nothing.\"\n{gone}"),
    );
    scratch.edit(
        FEATURE,
        "      text: \"Removed books are gone\"\n    criteria:\n",
        "      text: \"Removed books are gone\"\n    criteria:\n      - key: 00000000-0000-4000-8000-0000000000ac\n        name: \"A removed book stays removed.\"\n",
    );
    let document = json(&status(
        &scratch,
        &["--base", "develop", "--format", "json"],
        &git,
    ));
    assert!(keys(&document, "changed").is_empty(), "{document}");
    assert_eq!(
        keys(&document, "added"),
        ["00000000-0000-4000-8000-0000000000ac"]
    );
}

#[test]
fn the_base_commits_nodes_are_read_from_git_not_from_the_working_tree() {
    let scratch = Scratch::with_reading_list();
    scratch.edit(
        FEATURE,
        "Removing an absent book reports the book as absent and changes nothing.",
        "Removing an absent book reports it as absent.",
    );
    let git = based_on_now(&scratch);
    scratch.edit(
        FEATURE,
        "Removing an absent book reports it as absent.",
        "Removing an absent book reports the book as absent and changes nothing.",
    );
    let document = json(&status(
        &scratch,
        &["--base", "develop", "--format", "json"],
        &git,
    ));
    assert_eq!(keys(&document, "changed"), [ABSENT]);
}

#[test]
fn a_base_revision_that_names_no_commit_exits_1_naming_it_and_lists_no_node() {
    let scratch = Scratch::with_reading_list();
    let git = GitScenario::clean_on("feature");
    let output = status(&scratch, &["--base", "nosuch"], &git);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "`nosuch` names no commit\n");
    let document = json(&status(
        &scratch,
        &["--base", "nosuch", "--format", "json"],
        &git,
    ));
    assert_eq!(document["outcome"], "error");
    assert_eq!(document["errors"][0]["kind"], "base");
    assert!(document.get("base").is_none());
}

#[test]
fn a_base_revision_without_a_common_ancestor_exits_1_and_says_so() {
    let scratch = Scratch::with_reading_list();
    let git = GitScenario::clean_on("feature").unrelated("orphan");
    let output = status(&scratch, &["--base", "orphan"], &git);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "`orphan` shares no history with HEAD\n");
}

#[test]
fn a_base_tree_that_does_not_read_names_the_commit_and_the_place_of_the_error() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch).file_at(
        "develop",
        "spec/products/reading-list/shelf/broken.yaml",
        "ffff000000000000000000000000000000000000",
        "name: [broken\n",
    );
    let output = status(&scratch, &["--base", "develop"], &git);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    let error = stderr(&output);
    assert!(
        error.starts_with(&format!(
            "the tree at {COMMIT} does not read: spec/products/reading-list/shelf/broken.yaml:1:"
        )),
        "{error}"
    );
}

#[test]
fn a_reader_error_names_no_root_and_lists_no_node() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    fs::write(scratch.file(FEATURE), "name: [broken\n").unwrap();
    let output = status(&scratch, &["--base", "develop"], &git);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
}

#[test]
fn a_conflict_record_is_named_with_its_file_and_key_and_exits_1() {
    let scratch = Scratch::with_reading_list();
    let text = fs::read_to_string(scratch.file(FEATURE)).unwrap();
    fs::write(
        scratch.file(FEATURE),
        format!("{text}    conflicts:\n      - field: body\n        ours: \"Why books go.\"\n"),
    )
    .unwrap();
    let output = status(&scratch, &[], &GitScenario::clean_on("main"));
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    let error = stderr(&output);
    assert!(error.starts_with(&format!("spec/{FEATURE}")), "{error}");
    assert!(error.contains("conflict"), "{error}");
}

#[test]
fn the_outcome_is_reported_and_the_base_is_left_out_without_base() {
    let scratch = Scratch::with_reading_list();
    let document = json(&status(
        &scratch,
        &["--format", "json"],
        &GitScenario::clean_on("main"),
    ));
    assert_eq!(document["outcome"], "reported");
    assert!(document.get("base").is_none());
}
