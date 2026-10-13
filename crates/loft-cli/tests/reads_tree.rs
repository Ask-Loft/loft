//! Reading the tree: the rules every command tagged `reads-tree` follows on a reader error, run
//! against each command whose Feature in this repository's own `spec/` lists the tag; and the
//! converse, that every command that reports a read error lists it.

mod common;
mod conformance;
#[path = "common/help.rs"]
mod help;

use std::collections::BTreeMap;

use common::{Scratch, stderr, stdout};
use conformance::{is_pending, snapshot, tagged};

const READING_LIST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/v1/reading-list/loft-authored-state.json"
);

const PRODUCT: &str = "products/reading-list/product.yaml";

const TAG: &str = "reads-tree";

/// The arguments that run each implemented command, by the slug of its Feature.
const COMMANDS: [(&str, &[&str]); 8] = [
    ("loft-check", &["check"]),
    ("loft-fmt", &["fmt"]),
    ("loft-render", &["render"]),
    ("loft-import-v1", &["import", "v1", READING_LIST]),
    ("loft-publish", &["publish"]),
    ("loft-verify", &["verify"]),
    // Init refuses any existing `spec/` before reading it.
    ("loft-init", &["init"]),
    // The driver reads the three files git hands it, never the tree, so inputs that do not
    // exist report against the path they name.
    (
        "loft-merge-driver",
        &[
            "merge-driver",
            "base",
            "ours",
            "theirs",
            "spec/loftspec.yaml",
        ],
    ),
];

/// Tagged commands not implemented yet, by Feature slug and subcommand. Each must still be
/// unknown to `loft`; once it exists it moves to [`COMMANDS`] and is held to the rules.
const PENDING: [(&str, &str); 0] = [];

/// A scratch tree whose product file holds a comment, which the reader refuses at line 1.
fn broken() -> Scratch {
    let scratch = Scratch::with_reading_list();
    scratch.edit(PRODUCT, "\nslug:", " # a note\nslug:");
    scratch
}

/// What the command printed, standard output then standard error.
fn output_text(output: &std::process::Output) -> String {
    stdout(output) + &stderr(output)
}

#[test]
fn each_tagged_command_reports_the_error_changes_no_file_and_exits_1() {
    let commands: BTreeMap<&str, &[&str]> = COMMANDS.into_iter().collect();
    for slug in tagged(TAG) {
        if is_pending(&slug, &PENDING) {
            continue;
        }
        let args = commands
            .get(slug.as_str())
            .unwrap_or_else(|| panic!("{slug} lists `reads-tree` but COMMANDS has no entry"));
        let scratch = broken();
        let before = snapshot(&scratch.0);
        let output = scratch.loft(args);
        assert_eq!(output.status.code(), Some(1), "{slug}: the exit code");
        let text = output_text(&output);
        assert!(
            text.lines()
                .any(|line| line.starts_with(&format!("spec/{PRODUCT}:1:"))),
            "{slug}: the error at its file, line and column\n{text}"
        );
        assert!(snapshot(&scratch.0) == before, "{slug}: a file changed");
    }
}

#[test]
fn each_command_that_reports_a_read_error_lists_reads_tree() {
    let tagged = tagged(TAG);
    for (slug, args) in COMMANDS {
        let output = broken().loft(args);
        let reports = output_text(&output)
            .lines()
            .any(|line| line.starts_with(&format!("spec/{PRODUCT}:1:")));
        assert_eq!(
            reports,
            tagged.iter().any(|tag| tag == slug),
            "{slug}: reports a read error exactly when its Feature lists `reads-tree`"
        );
    }
}

#[test]
fn every_implemented_command_has_an_entry() {
    for name in help::subcommands() {
        assert!(
            COMMANDS.iter().any(|(_, args)| args[0] == name),
            "`loft {name}` has no entry in COMMANDS"
        );
    }
}
