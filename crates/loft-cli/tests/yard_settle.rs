//! Yard Log a merge left unsettled, settled by the next `loft yard` command that opens it: the
//! files of two copies merged one at a time, as git's merge driver merges them, then read.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use software_plant::Clock;
use yardlog::codec::ROOT;
use yardlog::merge::merge_file;

use crate::common::EPOCH;
use crate::yard::{ASK, BRANCH, Yard, block};

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
    fs::remove_dir_all(dir).unwrap();
    fs::create_dir_all(dir).unwrap();
    for (name, bytes) in files {
        fs::write(dir.join(name), bytes).unwrap();
    }
}

/// Merges the copies file by file, as git does with the driver, into the folder.
fn merge(
    dir: &Path,
    base: &BTreeMap<String, Vec<u8>>,
    ours: &BTreeMap<String, Vec<u8>>,
    theirs: &BTreeMap<String, Vec<u8>>,
) {
    let mut merged = BTreeMap::new();
    for name in ours.keys().chain(theirs.keys()) {
        let bytes = match (ours.get(name), theirs.get(name)) {
            (Some(o), Some(t)) => merge_file(name, base.get(name).map(Vec::as_slice), o, t)
                .unwrap()
                .into_bytes(),
            (Some(one), None) | (None, Some(one)) => one.clone(),
            (None, None) => unreachable!(),
        };
        merged.insert(name.clone(), bytes);
    }
    assert!(merged.contains_key(ROOT));
    put(dir, &merged);
}

/// Yard Log of two copies that each wrote a note `n1` after a common base, merged: this copy's
/// at revision 2 a minute after the base, the other's a minute later.
fn merged() -> Yard {
    let mut yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let dir = yard.folder(BRANCH);
    let base = files(&dir);
    let note = |text: &str| {
        format!(r#"[{{"id": "n1", "kind": "note", "stage": "shape", "markdown": "{text}"}}]"#)
    };
    yard.clock = Clock::fixed(EPOCH + 60);
    yard.write("s1", &note("Ours"), &[]);
    let ours = files(&dir);
    put(&dir, &base);
    yard.clock = Clock::fixed(EPOCH + 120);
    yard.write("s2", &note("Theirs"), &[]);
    let theirs = files(&dir);
    merge(&dir, &base, &ours, &theirs);
    yard.clock = Clock::fixed(EPOCH + 180);
    yard
}

#[test]
fn a_command_that_only_reads_yard_log_settles_it_as_one_that_writes_would() {
    let yard = merged();
    let read = yard.read("s1", &[]);
    assert_eq!(read["revision"], 4);
    assert_eq!(block(&read, "n1")["version"]["body"]["markdown"], "Ours");
    assert_eq!(
        block(&read, "n1-2")["version"]["body"]["markdown"],
        "Theirs"
    );
    let root = fs::read_to_string(yard.folder(BRANCH).join(ROOT)).unwrap();
    assert!(!root.contains("unsettledSince"), "{root}");
    assert_eq!(yard.read("s1", &[])["revision"], 4);
}

#[test]
fn a_read_since_a_revision_below_the_settling_returns_the_blocks_the_merge_brought_in() {
    let yard = merged();
    let read = yard.read("s1", &["--since", "2"]);
    let ids: Vec<&str> = read["stages"][0]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|block| block["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["n1-2"]);
    assert_eq!(block(&read, "n1-2")["reached"], 4);
}

#[test]
fn submission_history_reports_when_a_merged_resolution_reached_this_copy() {
    let mut yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let dir = yard.folder(BRANCH);
    let base = files(&dir);
    for minute in 1..=2 {
        yard.clock = Clock::fixed(EPOCH + minute * 60);
        yard.write(
            "s1",
            &format!(r#"[{{"id":"n{minute}","kind":"note","stage":"shape","markdown":"Ours"}}]"#),
            &[],
        );
    }
    let ours = files(&dir);
    put(&dir, &base);
    yard.clock = Clock::fixed(EPOCH + 180);
    yard.answer("s1", r#"[{"block":"q1","labels":["Top"]}]"#);
    let theirs = files(&dir);
    merge(&dir, &base, &ours, &theirs);
    yard.clock = Clock::fixed(EPOCH + 240);
    let read = yard.read("s1", &["--submissions"]);
    let sent = &read["sent"][0];
    assert_eq!(sent["state"], "answered");
    assert_eq!(sent["changes"][1]["act"]["revision"], 2);
    assert!(sent["reached"].as_u64().unwrap() >= 4);
    assert!(sent["reached"].as_u64().unwrap() <= read["revision"].as_u64().unwrap());
}
