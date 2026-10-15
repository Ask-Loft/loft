//! Writing the tree: every command tagged `writes-tree` finishes a committed save it finds and
//! discards an uncommitted one before it acts, run against each command whose Feature in this
//! repository's own `spec/` lists the tag; and the converse, that every command that changes a
//! file under `spec/` or beside it lists the tag.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Output;

use crate::commands::{COMMANDS, Fixture, PRODUCT, command, fixture};
use crate::common::{Scratch, stderr, stdout};
use crate::conformance::{tagged, tagged_commands};
use crate::snapshot::snapshot;
use loftspec::codec::SpecPath;
use loftspec::publish::{LOCK, Lock};
use loftspec_store::journal::{self, Changes, JOURNAL, RootPath};

const TAG: &str = "writes-tree";

/// The files the Format places beside `spec/`.
const BESIDE: [&str; 3] = ["v1-sidecar.json", "openspec-sidecar.json", "loft.lock.json"];

/// The body a planted save gives the Product, and the file it plants into an empty `spec/`.
const PLANTED_BODY: &str = "A planted body.";
const PLANTED_FILE: &str = "planted.yaml";

/// Runs `loft` over `fixture`, as [`command`] runs it.
fn run(scratch: &Scratch, fixture: Fixture, args: &[&str]) -> Output {
    command(scratch, fixture).args(args).output().unwrap()
}

/// The lock a planted save puts beside a clean tree: a previous publish at sequence 7, so the
/// next publish is sequence 8 if the save was finished first and 1 if it was discarded.
fn planted_lock() -> Vec<u8> {
    let zero = "0".repeat(64);
    let text = format!(
        r#"{{"format":"loftspec/1","algorithm":"sha256-jcs/1",
        "documentKey":"00000000-0000-4000-8000-000000000001",
        "stateRoot":"{zero}","contentRoot":"{zero}","bindingRoot":"{zero}",
        "publishSequence":7,"publishedAt":"2026-01-01T00:00:00Z"}}"#
    );
    Lock::read(text.as_bytes()).unwrap().to_bytes()
}

/// Plants an interrupted save, committed or not, that changes what the command reads: in an
/// empty directory, a file under `spec/`, so that `spec/` is no longer empty once the save is
/// finished; beside a clean tree, a previous lock, since a finished change under `spec/` would
/// be an uncommitted change that refuses a publish; otherwise the Product's body.
fn plant(scratch: &Scratch, fixture: Fixture, committed: bool) {
    let (path, bytes) = match fixture {
        Fixture::Empty => (
            RootPath::in_spec(&SpecPath::new(PLANTED_FILE)),
            b"planted\n".to_vec(),
        ),
        Fixture::Clean => (RootPath::beside(LOCK), planted_lock()),
        Fixture::Unkeyed => {
            let text = fs::read_to_string(scratch.file(PRODUCT)).unwrap();
            let planted = text.replacen("Keeps the books a reader means to read.", PLANTED_BODY, 1);
            assert_ne!(planted, text);
            (
                RootPath::in_spec(&SpecPath::new(PRODUCT)),
                planted.into_bytes(),
            )
        }
    };
    let mut changes = Changes::default();
    changes.write.insert(path, bytes);
    let prepared = journal::prepare(&scratch.0, &changes).unwrap();
    if committed {
        drop(prepared.commit().unwrap());
    }
}

/// Whether the planted save took effect.
fn planted(scratch: &Scratch, fixture: Fixture) -> bool {
    match fixture {
        Fixture::Empty => scratch.file(PLANTED_FILE).exists(),
        Fixture::Clean => {
            let lock = Lock::read(&fs::read(scratch.0.join(LOCK)).unwrap()).unwrap();
            lock.publish_sequence.get() == 8
        }
        Fixture::Unkeyed => fs::read_to_string(scratch.file(PRODUCT))
            .unwrap()
            .contains(PLANTED_BODY),
    }
}

/// Every file of the Format under the scratch directory: the tree and the files beside it.
fn format_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    snapshot(root)
        .into_iter()
        .filter(|(path, _)| path.starts_with("spec/") || BESIDE.contains(&path.as_str()))
        .collect()
}

/// Asserts the command exited with `code`, showing what it printed if not.
fn assert_exit(slug: &str, output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{slug}: the exit code\n{}{}",
        stdout(output),
        stderr(output)
    );
}

#[test]
fn each_tagged_command_finishes_a_committed_save_before_it_acts() {
    for (slug, args, kind) in tagged_commands(TAG) {
        let scratch = fixture(kind);
        plant(&scratch, kind, true);
        // In an empty directory the finished save leaves `spec/` not empty, which the command
        // must see before it writes.
        assert_exit(
            slug,
            &run(&scratch, kind, args),
            i32::from(kind == Fixture::Empty),
        );
        assert!(
            planted(&scratch, kind),
            "{slug}: the committed save was not finished first"
        );
        assert!(
            !scratch.0.join(JOURNAL).exists(),
            "{slug}: a journal is left"
        );
    }
}

#[test]
fn each_tagged_command_discards_an_uncommitted_save_before_it_acts() {
    for (slug, args, kind) in tagged_commands(TAG) {
        let scratch = fixture(kind);
        plant(&scratch, kind, false);
        assert_exit(slug, &run(&scratch, kind, args), 0);
        assert!(
            !planted(&scratch, kind),
            "{slug}: an uncommitted save took effect"
        );
        assert!(
            !scratch.0.join(JOURNAL).exists(),
            "{slug}: a journal is left"
        );
    }
}

#[test]
fn each_command_that_changes_a_file_of_the_format_lists_writes_tree() {
    let tagged = tagged(TAG);
    for (slug, args, kind) in COMMANDS {
        let scratch = fixture(kind);
        let before = format_files(&scratch.0);
        run(&scratch, kind, args);
        if format_files(&scratch.0) != before {
            assert!(
                tagged.iter().any(|tag| tag == slug),
                "{slug} changes a file under `spec/` or beside it but does not list `writes-tree`"
            );
        }
    }
}

#[test]
fn each_tagged_command_changes_a_file_of_the_format() {
    for (slug, args, kind) in tagged_commands(TAG) {
        let scratch = fixture(kind);
        let before = format_files(&scratch.0);
        assert_exit(slug, &run(&scratch, kind, args), 0);
        assert_ne!(format_files(&scratch.0), before, "{slug}: wrote nothing");
    }
}

/// The document key the manifest under `root` carries, read from its text so that a tree
/// without every key still answers.
fn document_key(root: &Path) -> String {
    let manifest = fs::read_to_string(root.join("spec/loftspec.yaml")).unwrap();
    let key = manifest
        .lines()
        .find_map(|line| line.strip_prefix("documentKey: "));
    key.expect("the manifest carries a document key").to_owned()
}

#[test]
fn no_tagged_command_changes_the_document_key_once_a_tree_is_written() {
    let commands = tagged_commands(TAG);
    let (first, later): (Vec<_>, Vec<_>) = commands
        .into_iter()
        .partition(|(_, _, kind)| *kind == Fixture::Empty);
    assert!(!first.is_empty() && !later.is_empty());
    // Each command that writes the first tree, then every other over the tree it wrote.
    for (slug, args, kind) in first {
        let scratch = fixture(kind);
        assert_exit(slug, &run(&scratch, kind, args), 0);
        let key = document_key(&scratch.0);
        for (next, args, kind) in &later {
            assert_exit(next, &run(&scratch, *kind, args), 0);
            assert_eq!(document_key(&scratch.0), key, "{next} after {slug}");
        }
    }
    // Each other command over its own fixture.
    for (slug, args, kind) in later {
        let scratch = fixture(kind);
        let key = document_key(&scratch.0);
        assert_exit(slug, &run(&scratch, kind, args), 0);
        assert_eq!(document_key(&scratch.0), key, "{slug}");
    }
}
