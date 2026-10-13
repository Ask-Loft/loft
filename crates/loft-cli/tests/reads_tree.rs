//! Reading the tree: the rules every command tagged `reads-tree` follows on a reader error, run
//! against each command whose Feature in this repository's own `spec/` lists the tag; and the
//! converse, that every command that reports a read error lists it.

#[path = "common/commands.rs"]
mod commands;
mod common;
mod conformance;
#[path = "common/snapshot.rs"]
mod snapshot;

use commands::COMMANDS;
use common::{Scratch, stderr, stdout};
use conformance::{tagged, tagged_commands};
use snapshot::snapshot;

const PRODUCT: &str = "products/reading-list/product.yaml";

const TAG: &str = "reads-tree";

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
    for (slug, args, _) in tagged_commands(TAG) {
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
    for (slug, args, _) in COMMANDS {
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
