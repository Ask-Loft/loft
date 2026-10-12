//! `loft publish` in a stopped merge or rebase (D-11, the `loft publish` command): in front of
//! the git simulator, it reads the stopped state before the lock, reads a lock that holds
//! conflict markers as none, prints what it settled and the git command that finishes, and
//! refuses what it cannot settle.

mod common;
#[path = "common/schema.rs"]
mod schema;

use std::fs;
use std::process::Output;

use common::{EPOCH, Scratch, stderr, stdout};
use loftspec::codec::SpecPath;
use loftspec::merge::merge_file;
use serde_json::{Value, json};
use software_plant::{Clock, GitScenario};

const FEATURE_F1: &str = "00000000-0000-4000-8000-00000000001e";
const FEATURE_F2: &str = "00000000-0000-4000-8000-00000000001f";
const FEATURE_F3: &str = "00000000-0000-4000-8000-000000000020";
const COMMIT: &str = "b97feb1503e857271adfbc4604548baba9ff4d2b";
const BLOB: &str = "20833b50d6fa1b4f9378bf129363513617191658";

/// A Reading List binding as `spec/labels.yaml` writes it.
fn binding(key: &str, label: &str) -> String {
    format!("  - key: {key}\n    label: {label}\n")
}

/// The scratch tree's bindings, the Features `F-1` and `F-3` unbound, with each of `bound` bound
/// where the key order puts it: before the binding of `F-2` or after it.
fn labels(scratch: &Scratch, bound: &[(&str, &str)]) -> String {
    let mut text = fs::read_to_string(scratch.file("labels.yaml")).unwrap();
    let anchor = binding(FEATURE_F2, "F-2");
    for (key, label) in bound {
        let placed = if *key < FEATURE_F2 {
            format!("{}{anchor}", binding(key, label))
        } else {
            format!("{anchor}{}", binding(key, label))
        };
        text = text.replacen(&anchor, &placed, 1);
    }
    text
}

/// A lock of the Reading List at publish sequence `sequence`, published in a clean repository.
fn lock(sequence: u64) -> String {
    let scratch = Scratch::with_reading_list();
    let clean = GitScenario::clean_on("main");
    let output = scratch.loft_with(&["publish"], Some(&clean), &Clock::fixed(EPOCH));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = fs::read_to_string(scratch.0.join("loft.lock.json")).unwrap();
    text.replacen(
        "\"publishSequence\": 1,",
        &format!("\"publishSequence\": {sequence},"),
        1,
    )
}

/// What stops: a merge, which hands the merge driver the local side as `ours`, or a rebase,
/// which hands it the pulled side as `ours`.
#[derive(Clone, Copy)]
enum Stop {
    Merge,
    Rebase,
}

/// The Reading List stopped in `stop` of `main`: the local side gave `F-4` to the Feature `F-3`
/// was, the pulled side gave it to the Feature `F-1` was; the lock holds conflict markers, and
/// the locks of the two commits are at sequences 4 and 6.
fn stopped(stop: Stop) -> (Scratch, GitScenario) {
    let scratch = Scratch::with_reading_list();
    scratch.edit("labels.yaml", &binding(FEATURE_F1, "F-1"), "");
    scratch.edit("labels.yaml", &binding(FEATURE_F3, "F-3"), "");
    let base = labels(&scratch, &[]);
    let local = labels(&scratch, &[(FEATURE_F3, "F-4")]);
    let pulled = labels(&scratch, &[(FEATURE_F1, "F-4")]);
    let (ours, theirs) = match stop {
        Stop::Merge => (&local, &pulled),
        Stop::Rebase => (&pulled, &local),
    };
    let at = SpecPath::new("labels.yaml");
    let merged = merge_file(&at, base.as_bytes(), ours.as_bytes(), theirs.as_bytes()).unwrap();
    fs::write(scratch.file("labels.yaml"), merged.bytes).unwrap();
    let markers = "<<<<<<< ours\n{}\n=======\n{}\n>>>>>>> theirs\n";
    fs::write(scratch.0.join("loft.lock.json"), markers).unwrap();
    let (head, other) = (lock(4), lock(6));
    let scenario = match stop {
        Stop::Merge => GitScenario::clean_on("main")
            .merging(COMMIT)
            .lock_at("MERGE_HEAD", &other),
        Stop::Rebase => GitScenario::clean_on("main")
            .detached()
            .rebasing(COMMIT, "refs/heads/main")
            .lock_at("REBASE_HEAD", &other),
    };
    let mut scenario = scenario
        .with("UU", "spec/labels.yaml")
        .lock_at("HEAD", &head);
    for path in ["loft.lock.json", "spec/labels.yaml"] {
        for stage in 1..=3 {
            scenario = scenario.unmerged("100644", BLOB, stage, path);
        }
    }
    (scratch, scenario)
}

fn publish(scratch: &Scratch, scenario: &GitScenario, args: &[&str]) -> Output {
    scratch.loft_with(args, Some(scenario), &Clock::fixed(EPOCH))
}

/// The lines a settling publish prints, but the one with the state root.
fn lines(output: &Output) -> Vec<String> {
    stdout(output)
        .lines()
        .map(|line| match line.split_once(", state root ") {
            Some((before, _)) => before.to_owned(),
            None => line.to_owned(),
        })
        .collect()
}

#[test]
fn a_publish_in_a_stopped_merge_prints_what_it_settled_and_how_to_finish() {
    let (scratch, scenario) = stopped(Stop::Merge);
    let output = publish(&scratch, &scenario, &["publish"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        lines(&output),
        [
            format!("F-5 {FEATURE_F3}"),
            format!("withdrew F-4 from {FEATURE_F3}, which now holds F-5"),
            "published sequence 7".to_owned(),
            "to finish: git add -A && git commit --no-edit".to_owned(),
        ]
    );
    let labels = fs::read_to_string(scratch.file("labels.yaml")).unwrap();
    assert!(labels.contains(&binding(FEATURE_F1, "F-4")), "{labels}");
    assert!(
        labels.contains(&format!(
            "{}    withdrawn:\n      - F-4\n",
            binding(FEATURE_F3, "F-5")
        )),
        "{labels}"
    );
    assert!(!labels.contains("conflicts:"), "{labels}");
    let check = scratch.loft(&["check", "--fail-on", "warning"]);
    assert_eq!(check.status.code(), Some(0), "{}", stdout(&check));
}

#[test]
fn a_publish_in_a_stopped_rebase_prints_git_rebase_continue() {
    let (scratch, scenario) = stopped(Stop::Rebase);
    let output = publish(&scratch, &scenario, &["publish"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let lines = lines(&output);
    assert_eq!(
        lines[1],
        format!("withdrew F-4 from {FEATURE_F3}, which now holds F-5")
    );
    assert_eq!(
        lines.last().unwrap(),
        "to finish: git add -A && git rebase --continue"
    );
}

#[test]
fn the_json_of_a_settling_publish_names_the_withdrawn_labels_and_the_finishing_command() {
    let (scratch, scenario) = stopped(Stop::Merge);
    let output = publish(&scratch, &scenario, &["publish", "--format", "json"]);
    let document: Value = serde_json::from_str(&stdout(&output)).unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/publish.json")).unwrap();
    assert_eq!(schema::violations(&document, &schema), Vec::<String>::new());
    assert_eq!(
        document["withdrawn"],
        json!([{"key": FEATURE_F3, "withdrawn": "F-4", "label": "F-5"}])
    );
    assert_eq!(document["rewritten"], json!([]));
    assert_eq!(document["finish"], "git add -A && git commit --no-edit");
    assert_eq!(document["publishSequence"], 7);
}

#[test]
fn a_second_publish_before_finishing_changes_nothing() {
    let (scratch, scenario) = stopped(Stop::Merge);
    let first = publish(&scratch, &scenario, &["publish"]);
    assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
    let before = fs::read(scratch.0.join("loft.lock.json")).unwrap();
    let again = publish(&scratch, &scenario, &["publish"]);
    assert_eq!(again.status.code(), Some(0), "{}", stderr(&again));
    assert!(
        stdout(&again).starts_with("nothing changed since publish sequence 7"),
        "{}",
        stdout(&again)
    );
    assert_eq!(fs::read(scratch.0.join("loft.lock.json")).unwrap(), before);
}

/// Every file under the scratch tree, by path.
fn snapshot(scratch: &Scratch) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut pending = vec![scratch.0.clone()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push((path.display().to_string(), fs::read(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}

#[test]
fn an_unmerged_code_file_is_refused_by_name_and_nothing_is_written() {
    let (scratch, scenario) = stopped(Stop::Merge);
    let scenario = scenario.unmerged("100644", BLOB, 2, "src/main.rs");
    let before = snapshot(&scratch);
    let output = publish(&scratch, &scenario, &["publish"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains(": src/main.rs;"),
        "{}",
        stderr(&output)
    );
    assert_eq!(snapshot(&scratch), before);
}

#[test]
fn a_rebase_of_another_branch_is_refused_and_nothing_is_written() {
    let (scratch, _) = stopped(Stop::Rebase);
    let scenario = GitScenario::clean_on("main")
        .detached()
        .rebasing(COMMIT, "refs/heads/feature")
        .with("UU", "spec/labels.yaml");
    let before = snapshot(&scratch);
    let output = publish(&scratch, &scenario, &["publish"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("`feature`"), "{}", stderr(&output));
    assert_eq!(snapshot(&scratch), before);
}

#[test]
fn unmerged_bindings_without_a_conflict_record_are_refused_with_the_way_out() {
    let (scratch, scenario) = stopped(Stop::Merge);
    // As git leaves the file when it could not run the driver: no conflict record.
    let merged = fs::read_to_string(scratch.file("labels.yaml")).unwrap();
    let plain = merged.split("conflicts:\n").next().unwrap();
    fs::write(scratch.file("labels.yaml"), plain).unwrap();
    let before = snapshot(&scratch);
    let output = publish(&scratch, &scenario, &["publish"]);
    assert_eq!(output.status.code(), Some(1));
    let refusal = stderr(&output);
    assert!(
        refusal.contains("git checkout -m spec/labels.yaml"),
        "{refusal}"
    );
    assert!(
        refusal.contains("`loft` is on git's search path"),
        "{refusal}"
    );
    assert_eq!(snapshot(&scratch), before);
}
