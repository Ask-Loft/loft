//! Writing the tree: every command tagged `writes-tree` finishes a committed save it finds and
//! discards an uncommitted one before it acts, run against each command whose Feature in this
//! repository's own `spec/` lists the tag; and the converse, that every command that changes a
//! file under `spec/` or beside it lists the tag.

mod common;
mod conformance;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Output;

use common::{EPOCH, Scratch, stderr, stdout};
use conformance::{is_pending, snapshot, tagged};
use loftspec::codec::SpecPath;
use loftspec::publish::{LOCK, Lock};
use loftspec_store::journal::{self, Changes, JOURNAL, RootPath};
use software_plant::{Clock, GitScenario};

const READING_LIST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/v1/reading-list/loft-authored-state.json"
);

const PRODUCT: &str = "products/reading-list/product.yaml";

const TAG: &str = "writes-tree";

/// The files the Format places beside `spec/`.
const BESIDE: [&str; 2] = ["v1-sidecar.json", "loft.lock.json"];

/// What a command runs over.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fixture {
    /// An empty directory.
    Empty,
    /// The Reading List with one key left out, so a formatter has a key to mint.
    Unkeyed,
    /// The Reading List as converted, in a clean repository on `main`.
    Clean,
}

/// The arguments that run each implemented command, by the slug of its Feature, and what it
/// runs over.
const COMMANDS: [(&str, &[&str], Fixture); 6] = [
    ("loft-check", &["check"], Fixture::Unkeyed),
    ("loft-fmt", &["fmt"], Fixture::Unkeyed),
    (
        "loft-render",
        &["render", "--markdown", "out/spec.md"],
        Fixture::Unkeyed,
    ),
    (
        "loft-import-v1",
        &["import", "v1", READING_LIST],
        Fixture::Empty,
    ),
    ("loft-publish", &["publish"], Fixture::Clean),
    ("loft-verify", &["verify"], Fixture::Clean),
];

/// Tagged commands not implemented yet, by Feature slug and subcommand. Each must still be
/// unknown to `loft`; once it exists it moves to [`COMMANDS`] and is held to the rules.
const PENDING: [(&str, &str); 1] = [("loft-init", "init")];

/// The body a planted save gives the Product, and the file it plants into an empty `spec/`.
const PLANTED_BODY: &str = "A planted body.";
const PLANTED_FILE: &str = "planted.yaml";

/// The lock a planted save puts beside a clean tree: a previous publish at sequence 7, so the
/// next publish is sequence 8 if the save was finished first and 1 if it was discarded.
/// Runs `loft` in a clean repository on `main` at [`EPOCH`].
fn on_main(scratch: &Scratch, args: &[&str]) -> Output {
    scratch.loft_with(
        args,
        Some(&GitScenario::clean_on("main")),
        &Clock::fixed(EPOCH),
    )
}

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

/// A scratch directory for a command.
fn fixture(fixture: Fixture) -> Scratch {
    let scratch = Scratch::with_reading_list();
    match fixture {
        Fixture::Empty => fs::remove_dir_all(scratch.0.join("spec")).unwrap(),
        Fixture::Unkeyed => scratch.edit(
            PRODUCT,
            "      - key: 00000000-0000-4000-8000-000000000032\n        name:",
            "      - name:",
        ),
        Fixture::Clean => {}
    }
    scratch
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

/// The implemented commands whose Features list the tag, after checking that each pending one
/// is still unknown to `loft`.
fn implemented_tagged() -> Vec<(&'static str, &'static [&'static str], Fixture)> {
    tagged(TAG)
        .into_iter()
        .filter(|slug| !is_pending(slug, &PENDING))
        .map(|slug| {
            COMMANDS
                .into_iter()
                .find(|(name, _, _)| *name == slug)
                .unwrap_or_else(|| panic!("{slug} lists `{TAG}` but COMMANDS has no entry"))
        })
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
    for (slug, args, kind) in implemented_tagged() {
        let scratch = fixture(kind);
        plant(&scratch, kind, true);
        // In an empty directory the finished save leaves `spec/` not empty, which the command
        // must see before it writes.
        assert_exit(
            slug,
            &on_main(&scratch, args),
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
    for (slug, args, kind) in implemented_tagged() {
        let scratch = fixture(kind);
        plant(&scratch, kind, false);
        assert_exit(slug, &on_main(&scratch, args), 0);
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
        on_main(&scratch, args);
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
    for (slug, args, kind) in implemented_tagged() {
        let scratch = fixture(kind);
        let before = format_files(&scratch.0);
        assert_exit(slug, &on_main(&scratch, args), 0);
        assert_ne!(format_files(&scratch.0), before, "{slug}: wrote nothing");
    }
}
