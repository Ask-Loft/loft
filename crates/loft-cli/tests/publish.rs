//! `loft publish` as a process (F-41, F-27 to F-30), run in front of the Software Plant's git
//! simulator and clock: what it prints, what it writes, and when it refuses.

use std::fs;
use std::process::Output;

use crate::common::{EPOCH, Scratch, stderr, stdout};
use crate::schema;
use crate::snapshot::{changed, snapshot};
use loftspec::codec::Files;
use loftspec::publish::{LOCK, Lock};
use loftspec_store::FileStore;
use serde_json::Value;
use software_plant::clock::EPOCH_ENV;
use software_plant::git::OPTIONAL_LOCKS_ENV;
use software_plant::{Clock, GitScenario};

/// Runs `loft` in a clean repository on `main` at [`EPOCH`].
fn on_main(scratch: &Scratch, args: &[&str]) -> Output {
    scratch.loft_with(
        args,
        Some(&GitScenario::clean_on("main")),
        &Clock::fixed(EPOCH),
    )
}

fn lock(scratch: &Scratch) -> Option<String> {
    fs::read_to_string(scratch.0.join(LOCK)).ok()
}

/// The files of the Format under the scratch directory: the tree and the lock.
fn format_files(scratch: &Scratch) -> (Files, Option<String>) {
    let files = FileStore::open(&scratch.0).unwrap().read_files().unwrap();
    (files, lock(scratch))
}

fn json(output: &Output) -> Value {
    schema::conforming(
        &output.stdout,
        include_str!("../../../schemas/publish.json"),
    )
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

#[test]
fn a_first_publish_writes_the_lock_and_prints_sequence_1_and_the_state_root() {
    let scratch = Scratch::with_reading_list();
    let output = on_main(&scratch, &["publish"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let state = Lock::read(lock(&scratch).unwrap().as_bytes())
        .unwrap()
        .state_root;
    assert_eq!(
        stdout(&output),
        format!("published sequence 1, state root {state}\n")
    );
}

#[test]
fn the_publish_time_is_the_one_the_clock_gives_in_utc() {
    let scratch = Scratch::with_reading_list();
    assert_eq!(on_main(&scratch, &["publish"]).status.code(), Some(0));
    assert!(
        lock(&scratch)
            .unwrap()
            .contains("\"publishedAt\": \"2026-01-01T00:00:00Z\"")
    );
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft_with(
        &["publish"],
        Some(&GitScenario::clean_on("main")),
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
fn a_publish_prints_each_allocated_label_with_its_key() {
    let scratch = Scratch::with_reading_list();
    let key = unlabel(&scratch, "AC-3");
    let output = on_main(&scratch, &["publish"]);
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
fn a_second_publish_with_nothing_changed_writes_nothing_and_exits_0() {
    let scratch = Scratch::with_reading_list();
    on_main(&scratch, &["publish"]);
    let before = format_files(&scratch);
    let output = scratch.loft_with(
        &["publish"],
        Some(&GitScenario::clean_on("main")),
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
fn uncommitted_changes_under_spec_refuse_naming_the_paths() {
    let scratch = Scratch::with_reading_list();
    let git = GitScenario::clean_on("main")
        .with(" M", "spec/tags.yaml")
        .with("??", "spec/products/new.yaml");
    let output = scratch.loft_with(&["publish"], Some(&git), &Clock::fixed(EPOCH));
    refuses(&scratch, &output, "spec/products/new.yaml, spec/tags.yaml");
}

#[test]
fn another_branch_refuses_naming_both_branches() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft_with(
        &["publish"],
        Some(&GitScenario::clean_on("develop")),
        &Clock::fixed(EPOCH),
    );
    refuses(
        &scratch,
        &output,
        "on branch `develop`, but the publish branch is `main`",
    );
}

#[test]
fn a_detached_head_refuses() {
    let scratch = Scratch::with_reading_list();
    let git = GitScenario::clean_on("main").detached();
    let output = scratch.loft_with(&["publish"], Some(&git), &Clock::fixed(EPOCH));
    refuses(
        &scratch,
        &output,
        "HEAD is detached, not on the publish branch `main`",
    );
}

#[test]
fn outside_a_git_repository_a_publish_refuses_and_writes_nothing() {
    let scratch = Scratch::with_reading_list();
    let before = format_files(&scratch);
    let output = scratch.loft_with(
        &["publish"],
        Some(&GitScenario::outside_repository()),
        &Clock::fixed(EPOCH),
    );
    refuses(&scratch, &output, "git repository");
    assert!(format_files(&scratch) == before, "a file changed");
}

#[test]
fn a_git_that_cannot_be_run_refuses_naming_git() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["publish"]);
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
    let output = scratch.loft_with(&["publish"], Some(&git), &Clock::fixed(EPOCH));
    refuses(&scratch, &output, "fatal: index file corrupt");
}

#[test]
fn an_invalid_lock_refuses_naming_the_lock_file() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.0.join(LOCK), "{}\n").unwrap();
    let output = on_main(&scratch, &["publish"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("loft.lock.json: not a valid lock record"));
    assert_eq!(lock(&scratch).as_deref(), Some("{}\n"));
}

#[test]
fn an_epoch_that_is_not_a_whole_number_of_seconds_refuses_naming_it() {
    let scratch = Scratch::with_reading_list();
    let output = scratch
        .command(Some(&GitScenario::clean_on("main")), &Clock::system())
        .arg("publish")
        .env(EPOCH_ENV, "soon")
        .output()
        .unwrap();
    refuses(&scratch, &output, "SOURCE_DATE_EPOCH");
}

#[test]
fn a_validation_error_refuses_listing_the_errors() {
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
    let output = on_main(&scratch, &["publish"]);
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
        Some(&GitScenario::outside_repository()),
        &Clock::fixed(EPOCH),
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).starts_with("spec/products/reading-list/product.yaml:1:"));
}

#[test]
fn the_json_results_match_the_publish_schema() {
    let scratch = Scratch::with_reading_list();
    let key = unlabel(&scratch, "AC-3");
    let published = json(&on_main(&scratch, &["publish", "--format", "json"]));
    assert_eq!(published["outcome"], "published");
    assert_eq!(published["publishSequence"], 1);
    assert_eq!(published["allocated"][0]["label"], "AC-8");
    assert_eq!(published["allocated"][0]["key"], key.as_str());
    let unchanged = json(&on_main(&scratch, &["publish", "--format", "json"]));
    assert_eq!(unchanged["outcome"], "unchanged");
    let refused = json(&scratch.loft_with(
        &["publish", "--format", "json"],
        Some(&GitScenario::clean_on("develop")),
        &Clock::fixed(EPOCH),
    ));
    assert_eq!(refused["outcome"], "error");
    assert_eq!(refused["errors"][0]["kind"], "refused");
}

#[test]
fn a_publish_reads_the_repository_without_taking_any_of_gits_locks() {
    let scratch = Scratch::with_reading_list();
    let plant = scratch.0.join(".plant");
    fs::create_dir_all(&plant).unwrap();
    let installed = GitScenario::clean_on("main").install(&plant);
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    installed.apply(&mut command).env_remove(OPTIONAL_LOCKS_ENV);
    let output = command.arg("publish").output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(installed.locks(), Vec::<String>::new());
}

#[test]
fn a_publish_prints_and_writes_each_citation_of_a_withdrawn_label_it_rewrites() {
    let scratch = Scratch::with_reading_list();
    scratch.edit(
        "labels.yaml",
        "    label: F-2\n",
        "    label: F-2\n    withdrawn:\n      - F-9\n",
    );
    let feature = "products/reading-list/shelf/add-a-book.yaml";
    scratch.edit(feature, "F-2 removes them.", "F-9 removes them.");
    let output = on_main(&scratch, &["publish"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output).starts_with(
            "rewrote F-9 as F-2 in 00000000-0000-4000-8000-00000000001e\npublished sequence 1, "
        ),
        "{}",
        stdout(&output)
    );
    let text = fs::read_to_string(scratch.file(feature)).unwrap();
    assert!(text.contains("F-2 removes them."), "{text}");
    let again = on_main(&scratch, &["publish", "--format", "json"]);
    let document = json(&again);
    assert_eq!(document["outcome"], "unchanged");
    assert_eq!(document["rewritten"], serde_json::json!([]));
}

#[test]
fn the_json_of_a_publish_lists_the_citations_it_rewrote() {
    let scratch = Scratch::with_reading_list();
    scratch.edit(
        "labels.yaml",
        "    label: F-2\n",
        "    label: F-2\n    withdrawn:\n      - F-9\n",
    );
    scratch.edit(
        "products/reading-list/shelf/add-a-book.yaml",
        "F-2 removes them.",
        "F-9 removes them.",
    );
    let document = json(&on_main(&scratch, &["publish", "--format", "json"]));
    assert_eq!(document["schema"], "loft-publish/1");
    assert_eq!(
        document["rewritten"],
        serde_json::json!([{
            "key": "00000000-0000-4000-8000-00000000001e",
            "withdrawn": "F-9",
            "label": "F-2",
        }])
    );
}

/// Runs a publish through `run` over the Reading List with a label to allocate, once `prepare`
/// has made it fail; asserts it exits 1 and writes nothing, and gives its output.
fn fails_writing_nothing(
    prepare: impl FnOnce(&Scratch),
    run: impl FnOnce(&Scratch) -> Output,
) -> Output {
    let scratch = Scratch::with_reading_list();
    unlabel(&scratch, "AC-3");
    prepare(&scratch);
    let before = snapshot(&scratch.0);
    let output = run(&scratch);
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    assert_eq!(snapshot(&scratch.0), before);
    output
}

#[test]
fn a_git_that_cannot_be_run_writes_nothing() {
    fails_writing_nothing(|_| {}, |scratch| scratch.loft(&["publish"]));
}

#[test]
fn an_invalid_lock_writes_nothing() {
    fails_writing_nothing(
        |scratch| fs::write(scratch.0.join(LOCK), "{}\n").unwrap(),
        |scratch| on_main(scratch, &["publish"]),
    );
}

#[test]
fn a_publish_writes_only_the_lock_and_the_bindings_and_no_projection() {
    let scratch = Scratch::with_reading_list();
    unlabel(&scratch, "AC-3");
    let before = snapshot(&scratch.0);
    let output = on_main(&scratch, &["publish"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let after = snapshot(&scratch.0);
    assert_eq!(changed(&before, &after), [LOCK, "spec/labels.yaml"]);
    assert!(before.keys().all(|path| after.contains_key(path)));
    assert!(!scratch.0.join("docs").exists());
}

#[test]
fn a_publish_whose_journal_cannot_be_written_changes_no_file() {
    // The journal lives under `.loft/`, which a file of that name keeps from being created.
    let output = fails_writing_nothing(
        |scratch| fs::write(scratch.0.join(".loft"), "not a directory\n").unwrap(),
        |scratch| on_main(scratch, &["publish"]),
    );
    assert!(stderr(&output).contains(".loft"), "{}", stderr(&output));
}

#[test]
fn a_node_deleted_formatted_restored_and_formatted_again_is_published_with_its_label_back() {
    let scratch = Scratch::with_reading_list();
    let feature = scratch.file("products/reading-list/shelf/add-a-book.yaml");
    let original = fs::read_to_string(&feature).unwrap();
    let deleted = "00000000-0000-4000-8000-00000000002a";
    let criterion = format!(
        "      - key: {deleted}\n        name: \"A book added without a title is refused with an error naming the title.\"\n"
    );
    fs::write(&feature, original.replace(&criterion, "")).unwrap();
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0));
    fs::write(&feature, &original).unwrap();
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0));
    let output = on_main(&scratch, &["publish"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output).contains(&format!("AC-2 {deleted} (given back)\n")),
        "{}",
        stdout(&output)
    );
    let labels = fs::read_to_string(scratch.file("labels.yaml")).unwrap();
    assert!(
        labels.contains(&format!("  - key: {deleted}\n    label: AC-2\n  - key: ")),
        "{labels}"
    );
}

/// The Reading List marked connected.
fn connected() -> Scratch {
    let scratch = Scratch::with_reading_list();
    let manifest = scratch.file("loftspec.yaml");
    let text = fs::read_to_string(&manifest).unwrap();
    let marked = text.replacen("products:\n", "connected: true\nproducts:\n", 1);
    fs::write(&manifest, marked).unwrap();
    scratch
}

#[test]
fn loft_publish_never_publishes_as_the_remote_so_a_connected_document_is_refused() {
    for unlabelled in [false, true] {
        let scratch = connected();
        if unlabelled {
            unlabel(&scratch, "AC-2");
        }
        let before = snapshot(&scratch.0);
        let output = on_main(&scratch, &["publish", "--format", "json"]);
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        let document = json(&output);
        assert_eq!(document["outcome"], "error");
        let message = document["errors"][0]["message"].as_str().unwrap();
        for said in [
            "connected",
            "only through its remote",
            "labels come from its remote",
        ] {
            assert!(message.contains(said), "{message}");
        }
        assert_eq!(
            changed(&before, &snapshot(&scratch.0)),
            Vec::<String>::new()
        );
    }
}
