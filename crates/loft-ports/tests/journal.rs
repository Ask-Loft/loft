//! The journaled save over plain paths: the changed files are staged in a journal directory the
//! caller names, which may lie outside the directory the files are saved under; a save leaves
//! either every file as it was or every file as written, and recovery finishes a committed save
//! or discards an uncommitted one.

use std::collections::BTreeMap;
use std::fs;

use crate::common::{Scratch, snapshot};
use loft_ports::PortError;
use loft_ports::journal::{Changes, Journal, RelPath};

/// The files saved under `root/`, the journal in `elsewhere/save`.
fn layout() -> (Scratch, Journal) {
    let scratch = Scratch::new();
    scratch.put("root/.yard/main/yardlog.yaml", "root 1\n");
    scratch.put("root/.yard/main/a.yaml", "a 1\n");
    scratch.put("root/README.md", "readme\n");
    // The journal's directory lies beside a lock, as Yard Log's does.
    scratch.put("elsewhere/lock", "");
    let journal = Journal::new(scratch.0.join("root"), scratch.0.join("elsewhere/save"));
    (scratch, journal)
}

fn path(text: &str) -> RelPath {
    RelPath::new(text)
}

fn changes() -> Changes {
    let mut changes = Changes::default();
    changes
        .write
        .insert(path(".yard/main/yardlog.yaml"), b"root 2\n".to_vec());
    changes
        .write
        .insert(path(".yard/main/b.yaml"), b"b 1\n".to_vec());
    changes.remove.insert(path(".yard/main/a.yaml"));
    changes
}

fn written() -> BTreeMap<String, Vec<u8>> {
    [
        ("elsewhere/", ""),
        ("elsewhere/lock", ""),
        ("root/", ""),
        ("root/README.md", "readme\n"),
        ("root/.yard/", ""),
        ("root/.yard/main/", ""),
        ("root/.yard/main/yardlog.yaml", "root 2\n"),
        ("root/.yard/main/b.yaml", "b 1\n"),
    ]
    .into_iter()
    .map(|(path, text)| (path.to_owned(), text.as_bytes().to_vec()))
    .collect()
}

#[test]
fn a_save_through_a_journal_elsewhere_writes_every_file_and_leaves_no_journal() {
    let (scratch, journal) = layout();
    journal.save(&changes()).unwrap();
    assert_eq!(scratch.snapshot(), written());
}

#[test]
fn a_journal_elsewhere_puts_nothing_of_itself_under_the_root() {
    let (scratch, journal) = layout();
    let prepared = journal.prepare(&changes()).unwrap();
    let under_root = snapshot(&scratch.0.join("root"));
    assert!(
        under_root.keys().all(|path| !path.contains("save")),
        "{under_root:?}"
    );
    assert!(scratch.0.join("elsewhere/save").is_dir());
    prepared.commit().unwrap().finish().unwrap();
    assert!(!scratch.0.join("elsewhere/save").exists());
}

#[test]
fn an_empty_save_creates_no_journal() {
    let (scratch, journal) = layout();
    let before = scratch.snapshot();
    journal.save(&Changes::default()).unwrap();
    assert_eq!(scratch.snapshot(), before);
}

#[test]
fn a_save_interrupted_after_its_commit_is_finished_by_recovery() {
    let (scratch, journal) = layout();
    drop(journal.prepare(&changes()).unwrap().commit().unwrap());
    assert_eq!(
        fs::read(scratch.0.join("root/.yard/main/yardlog.yaml")).unwrap(),
        b"root 1\n"
    );
    journal.recover().unwrap();
    assert_eq!(scratch.snapshot(), written());
}

#[test]
fn a_save_interrupted_before_its_commit_is_discarded_by_recovery() {
    let (scratch, journal) = layout();
    let before = scratch.snapshot();
    drop(journal.prepare(&changes()).unwrap());
    journal.recover().unwrap();
    assert_eq!(scratch.snapshot(), before);
}

#[test]
fn recovery_twice_finishes_the_save_once() {
    let (scratch, journal) = layout();
    drop(journal.prepare(&changes()).unwrap().commit().unwrap());
    journal.recover().unwrap();
    journal.recover().unwrap();
    assert_eq!(scratch.snapshot(), written());
}

#[test]
fn a_save_while_another_is_pending_is_refused_and_writes_nothing() {
    let (scratch, journal) = layout();
    drop(journal.prepare(&changes()).unwrap());
    let before = scratch.snapshot();
    let error = journal.save(&changes()).unwrap_err();
    assert!(matches!(error, PortError::Pending(_)), "{error}");
    assert_eq!(scratch.snapshot(), before);
}

#[test]
fn a_journal_of_an_unknown_format_is_refused_by_recovery() {
    let (scratch, journal) = layout();
    scratch.put(
        "elsewhere/save/plan.json",
        r#"{"format":"other/9","write":[],"remove":[]}"#,
    );
    let error = journal.recover().unwrap_err();
    assert!(
        matches!(&error, PortError::Journal(_, message) if message.contains("other/9")),
        "{error}"
    );
}

#[test]
fn empty_directories_are_removed_only_below_the_floor() {
    let scratch = Scratch::new();
    scratch.put("spec/products/a/product.yaml", "a\n");
    scratch.put("only/one.yaml", "x\n");
    let journal = Journal::new(scratch.0.clone(), scratch.0.join(".loft/save")).keeping("spec");
    let mut changes = Changes::default();
    changes.remove.insert(path("spec/products/a/product.yaml"));
    changes.remove.insert(path("only/one.yaml"));
    journal.save(&changes).unwrap();
    let left: Vec<String> = scratch.snapshot().into_keys().collect();
    assert_eq!(left, ["only/", "spec/"]);
}
