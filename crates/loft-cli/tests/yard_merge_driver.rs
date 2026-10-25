//! `loft merge-driver <base> <ours> <theirs> <path>` on the files of Yard Log under `.yard/`, as
//! git runs it: each file joined by key, a session's file taken whole, no conflict record ever,
//! and a copy that does not read refused with `<ours>` unchanged.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Output;

use serde_json::{Value, json};
use software_plant::Clock;
use yardlog::codec::ROOT;

use crate::common::{EPOCH, stderr};
use crate::schema;
use crate::yard::{ASK, BRANCH, Yard};

/// The three temporary files git hands the driver, at the top of the worktree.
const INPUTS: [&str; 3] = [".merge_file_base", ".merge_file_ours", ".merge_file_theirs"];

/// The files of a folder, by name.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.path().is_file())
        .map(|entry| {
            let name = entry.file_name().into_string().unwrap();
            (name, fs::read(entry.path()).unwrap())
        })
        .collect()
}

/// Replaces a folder's files with `files`.
fn put(dir: &Path, files: &BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            fs::remove_file(path).unwrap();
        }
    }
    for (name, bytes) in files {
        fs::write(dir.join(name), bytes).unwrap();
    }
}

/// The name of the one stage's file among `files`.
fn stage_file(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut names = files.keys().filter(|name| *name != ROOT);
    let name = names.next().expect("a stage's file").clone();
    assert!(names.next().is_none());
    name
}

/// The copies of Yard Log on [`BRANCH`] in the base, after this copy wrote a note `n1` and after
/// the other copy wrote its own `n1`, a minute apart; the folder is left as this copy's.
fn copies(yard: &mut Yard) -> [BTreeMap<String, Vec<u8>>; 3] {
    yard.write("s1", ASK, &[]);
    let dir = yard.folder(BRANCH);
    let base = files(&dir);
    let note = |text: &str| {
        format!(r#"[{{"id": "n1", "kind": "note", "stage": "shape", "markdown": "{text}"}}]"#)
    };
    yard.clock = Clock::fixed(EPOCH + 120);
    yard.write("s2", &note("Theirs"), &[]);
    let theirs = files(&dir);
    put(&dir, &base);
    yard.clock = Clock::fixed(EPOCH + 60);
    yard.write("s1", &note("Ours"), &[]);
    let ours = files(&dir);
    [base, ours, theirs]
}

/// Writes the three inputs and runs the driver for `path`, printing JSON.
fn driver(yard: &Yard, inputs: [&[u8]; 3], path: &str) -> Output {
    for (name, bytes) in INPUTS.iter().zip(inputs) {
        fs::write(yard.scratch.0.join(name), bytes).unwrap();
    }
    let [base, ours, theirs] = INPUTS;
    let args = ["merge-driver", base, ours, theirs, path, "--format", "json"];
    yard.scratch.loft(&args)
}

/// The one JSON document a run printed, held to the `loft merge-driver` schema.
fn json_of(output: &Output) -> Value {
    schema::conforming(
        &output.stdout,
        include_str!("../../../schemas/merge-driver.json"),
    )
}

fn ours(yard: &Yard) -> String {
    fs::read_to_string(yard.scratch.0.join(INPUTS[1])).unwrap()
}

/// The path git names a file of the branch's Yard Log by.
fn path(name: &str) -> String {
    format!(".yard/{BRANCH}/{name}")
}

/// The lines of `text` that give an id.
fn ids(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.trim_start().starts_with("id: "))
        .collect()
}

#[test]
fn a_stage_s_file_under_yard_is_merged_by_key_and_exits_0_with_no_conflict() {
    let mut yard = Yard::new();
    let [base, ours_files, theirs] = copies(&mut yard);
    let name = stage_file(&ours_files);
    let output = driver(
        &yard,
        [&base[&name], &ours_files[&name], &theirs[&name]],
        &path(&name),
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document = json_of(&output);
    assert_eq!(document["outcome"], "merged");
    assert_eq!(document["path"], json!(path(&name)));
    assert_eq!(document["conflicts"], 0);
    assert_eq!(document["disputed"], json!([]));
    let merged = ours(&yard);
    assert!(
        merged.contains("Ours") && merged.contains("Theirs"),
        "{merged}"
    );
    assert!(!merged.contains("<<<<<<<"), "{merged}");
    // Both notes keep the id each copy wrote: the driver of a stage's file changes no id.
    assert_eq!(
        ids(&merged)
            .iter()
            .filter(|line| line.trim() == "id: n1")
            .count(),
        2,
        "{merged}"
    );
}

#[test]
fn the_root_merged_by_the_driver_is_a_revision_above_both_and_unsettled_since_this_copy_s() {
    let mut yard = Yard::new();
    let [base, ours_files, theirs] = copies(&mut yard);
    let output = driver(
        &yard,
        [&base[ROOT], &ours_files[ROOT], &theirs[ROOT]],
        &path(ROOT),
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let merged = ours(&yard);
    assert!(merged.contains("\nrevision: 3\n"), "{merged}");
    assert!(merged.contains("\nunsettledSince: 2\n"), "{merged}");
}

#[test]
fn two_yard_logs_opened_apart_join_over_an_empty_base_under_the_earlier_key() {
    let earlier = Yard::new();
    earlier.write("s1", ASK, &[]);
    let mut later = Yard::new();
    later.clock = Clock::fixed(EPOCH + 60);
    later.write("s2", ASK, &[]);
    let root = |yard: &Yard| fs::read(yard.folder(BRANCH).join(ROOT)).unwrap();
    let key = |text: &[u8]| {
        String::from_utf8(text.to_vec())
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix("key: ").map(str::to_owned))
            .unwrap()
    };
    let (first, second) = (root(&earlier), root(&later));
    let output = driver(&later, [b"", &second, &first], &path(ROOT));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let merged = ours(&later);
    assert_eq!(key(merged.as_bytes()), key(&first), "{merged}");
    assert!(merged.contains(&format!("- {}", key(&second))), "{merged}");
}

/// A session's file, `tokens/s1.yaml`, whose last event ended `seconds` after [`EPOCH`].
fn session_file(seconds: u64) -> Vec<u8> {
    let at = yardlog::Time::from_unix_seconds(EPOCH + seconds).unwrap();
    format!("session: s1\nlastEnded: {at}\nrequests: {seconds}\n").into_bytes()
}

#[test]
fn a_session_s_file_is_taken_whole_from_the_copy_whose_last_event_ended_later() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let tokens = path("tokens/s1.yaml");
    let (earlier, later) = (session_file(60), session_file(120));
    let output = driver(&yard, [&session_file(0), &earlier, &later], &tokens);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(json_of(&output)["conflicts"], 0);
    assert_eq!(ours(&yard).as_bytes(), later.as_slice());
    let activity = path("activity/s1.yaml");
    let output = driver(&yard, [&session_file(0), &later, &earlier], &activity);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(ours(&yard).as_bytes(), later.as_slice());
}

#[test]
fn of_two_copies_of_a_session_s_file_whose_last_events_ended_at_one_time_this_copy_is_taken() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let mine = session_file(60);
    let mut other = session_file(60);
    other.extend_from_slice(b"stages: 1\n");
    let output = driver(
        &yard,
        [&session_file(0), &mine, &other],
        &path("tokens/s1.yaml"),
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(ours(&yard).as_bytes(), mine.as_slice());
}

#[test]
fn a_stage_s_file_of_a_branch_whose_name_ends_in_tokens_is_merged_by_key() {
    let branch = "x/tokens";
    let mut yard = Yard::on(branch);
    yard.write("s1", ASK, &[]);
    let dir = yard.folder(branch);
    let base = files(&dir);
    let name = stage_file(&base);
    let note = |id: &str| {
        format!(r#"[{{"id": "{id}", "kind": "note", "stage": "shape", "markdown": "A"}}]"#)
    };
    yard.clock = Clock::fixed(EPOCH + 60);
    yard.write("s1", &note("n1"), &[]);
    let ours_files = files(&dir);
    put(&dir, &base);
    yard.write("s2", &note("n2"), &[]);
    let theirs = files(&dir);
    put(&dir, &ours_files);
    let at = format!(".yard/{branch}/{name}");
    let output = driver(
        &yard,
        [&base[&name], &ours_files[&name], &theirs[&name]],
        &at,
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let merged = ours(&yard);
    assert!(
        merged.contains("id: n1") && merged.contains("id: n2"),
        "{merged}"
    );
}

#[test]
fn a_copy_that_does_not_read_as_yardlog_writes_nothing_and_exits_1_naming_the_copy_and_where() {
    let mut yard = Yard::new();
    let [base, ours_files, _] = copies(&mut yard);
    let name = stage_file(&ours_files);
    let broken = b"key: 01a00000-0000-7000-8000-000000000000\nid: [shape\n";
    let at = path(&name);
    let output = driver(&yard, [&base[&name], &ours_files[&name], broken], &at);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(ours(&yard).as_bytes(), ours_files[&name].as_slice());
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["outcome"], "error");
    let message = stderr(&output);
    assert!(message.starts_with(&format!("{at}:2:")), "{message}");
    assert!(message.contains(": theirs: "), "{message}");
}
