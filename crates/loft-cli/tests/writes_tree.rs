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

use common::{Scratch, stderr, stdout};
use conformance::{is_pending, snapshot, tagged};
use loftspec::codec::SpecPath;
use loftspec_store::journal::{self, Changes, JOURNAL, RootPath};

const READING_LIST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/v1/reading-list/loft-authored-state.json"
);

const PRODUCT: &str = "products/reading-list/product.yaml";

const TAG: &str = "writes-tree";

/// The files the Format places beside `spec/`.
const BESIDE: [&str; 2] = ["v1-sidecar.json", "loft.lock.json"];

/// The arguments that run each implemented command, by the slug of its Feature, and whether it
/// runs in an empty directory rather than over the Reading List.
const COMMANDS: [(&str, &[&str], bool); 4] = [
    ("loft-check", &["check"], false),
    ("loft-fmt", &["fmt"], false),
    (
        "loft-render",
        &["render", "--markdown", "out/spec.md"],
        false,
    ),
    ("loft-import-v1", &["import", "v1", READING_LIST], true),
];

/// Tagged commands not implemented yet, by Feature slug and subcommand. Each must still be
/// unknown to `loft`; once it exists it moves to [`COMMANDS`] and is held to the rules.
const PENDING: [(&str, &str); 2] = [("loft-init", "init"), ("loft-publish", "publish")];

/// The body a planted save gives the Product, and the file it plants into an empty `spec/`.
const PLANTED_BODY: &str = "A planted body.";
const PLANTED_FILE: &str = "planted.yaml";

/// A scratch directory for a command: empty, or the Reading List with one key left out, so a
/// formatter has a key to mint.
fn fixture(empty: bool) -> Scratch {
    let scratch = Scratch::with_reading_list();
    if empty {
        fs::remove_dir_all(scratch.0.join("spec")).unwrap();
    } else {
        scratch.edit(
            PRODUCT,
            "      - key: 00000000-0000-4000-8000-000000000032\n        name:",
            "      - name:",
        );
    }
    scratch
}

/// Plants an interrupted save, committed or not, that changes what the command reads: in a
/// tree, the Product's body; in an empty directory, a file under `spec/`, so that `spec/` is
/// no longer empty once the save is finished.
fn plant(scratch: &Scratch, empty: bool, committed: bool) {
    let mut changes = Changes::default();
    let (path, bytes) = if empty {
        (PLANTED_FILE.to_owned(), b"planted\n".to_vec())
    } else {
        let text = fs::read_to_string(scratch.file(PRODUCT)).unwrap();
        let planted = text.replacen("Keeps the books a reader means to read.", PLANTED_BODY, 1);
        assert_ne!(planted, text);
        (PRODUCT.to_owned(), planted.into_bytes())
    };
    changes
        .write
        .insert(RootPath::in_spec(&SpecPath::new(&path)), bytes);
    let prepared = journal::prepare(&scratch.0, &changes).unwrap();
    if committed {
        drop(prepared.commit().unwrap());
    }
}

/// Whether the planted save took effect.
fn planted(scratch: &Scratch, empty: bool) -> bool {
    if empty {
        scratch.file(PLANTED_FILE).exists()
    } else {
        fs::read_to_string(scratch.file(PRODUCT))
            .unwrap()
            .contains(PLANTED_BODY)
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
fn implemented_tagged() -> Vec<(&'static str, &'static [&'static str], bool)> {
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
    for (slug, args, empty) in implemented_tagged() {
        let scratch = fixture(empty);
        plant(&scratch, empty, true);
        // In an empty directory the finished save leaves `spec/` not empty, which the command
        // must see before it writes.
        assert_exit(slug, &scratch.loft(args), i32::from(empty));
        assert!(
            planted(&scratch, empty),
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
    for (slug, args, empty) in implemented_tagged() {
        let scratch = fixture(empty);
        plant(&scratch, empty, false);
        assert_exit(slug, &scratch.loft(args), 0);
        assert!(
            !planted(&scratch, empty),
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
    for (slug, args, empty) in COMMANDS {
        let scratch = fixture(empty);
        let before = format_files(&scratch.0);
        scratch.loft(args);
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
    for (slug, args, empty) in implemented_tagged() {
        let scratch = fixture(empty);
        let before = format_files(&scratch.0);
        assert_exit(slug, &scratch.loft(args), 0);
        assert_ne!(format_files(&scratch.0), before, "{slug}: wrote nothing");
    }
}
