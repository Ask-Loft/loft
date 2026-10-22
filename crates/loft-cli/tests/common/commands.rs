//! Every command `loft` implements, once: the slug of its Feature in this repository's own
//! `spec/`, the arguments that run it, and the fixture it runs over. The tests that hold every
//! command to a rule read it; the first argument is the subcommand, which names its schema.

use std::fs;
use std::process::Command;

use crate::common::{EPOCH, Scratch};
use software_plant::{Clock, GitScenario};

/// The Product's file, from which the unkeyed fixture leaves a key out.
pub const PRODUCT: &str = "products/reading-list/product.yaml";

const READING_LIST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/v1/reading-list/loft-authored-state.json"
);

/// The `OpenSpec` directory of a faithful `OpenSpec` project.
const OPENSPEC: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/openspec/faithful/openspec"
);

/// A batch of blocks for `loft yard write`: a stage, a question and a note.
pub const YARD_BATCH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/yard/batch.json"
);

/// An answer to the batch's question, for `loft yard answer`.
pub const YARD_ANSWERS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/yard/answers.json"
);

/// What a command runs over.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fixture {
    /// An empty directory.
    Empty,
    /// The Reading List with one key left out, so a formatter has a key to mint.
    Unkeyed,
    /// The Reading List as converted, in a clean repository on `main`.
    Clean,
}

/// A scratch directory for a command.
pub fn fixture(fixture: Fixture) -> Scratch {
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

/// The `loft` command over `fixture` at [`EPOCH`]: in a clean repository on `main` for the
/// clean fixture, with no `git` otherwise.
pub fn command(scratch: &Scratch, fixture: Fixture) -> Command {
    let git = (fixture == Fixture::Clean).then(|| GitScenario::clean_on("main"));
    scratch.command(git.as_ref(), &Clock::fixed(EPOCH))
}

/// A command: the slug of its Feature, the arguments that run it, its fixture, and the name of
/// its schema under `schemas/`.
pub type Entry = (&'static str, &'static [&'static str], Fixture, &'static str);

/// Each command by the slug of its Feature, with the arguments that run it, its fixture and its
/// schema. `import v1` keeps the schema `import`, released before a second import source existed.
pub const COMMANDS: [Entry; 25] = [
    ("loft-check", &["check"], Fixture::Unkeyed, "check"),
    ("loft-fmt", &["fmt"], Fixture::Unkeyed, "fmt"),
    (
        "loft-render",
        &["render", "--markdown", "out/spec.md"],
        Fixture::Unkeyed,
        "render",
    ),
    (
        "loft-import-v1",
        &["import", "v1", READING_LIST],
        Fixture::Empty,
        "import",
    ),
    (
        "loft-import-openspec",
        &["import", "openspec", OPENSPEC],
        Fixture::Empty,
        "import-openspec",
    ),
    ("loft-publish", &["publish"], Fixture::Clean, "publish"),
    ("loft-verify", &["verify"], Fixture::Clean, "verify"),
    ("loft-status", &["status"], Fixture::Clean, "status"),
    ("loft-find", &["find", "shelf"], Fixture::Clean, "find"),
    ("loft-review", &["review"], Fixture::Clean, "review"),
    ("loft-terms", &["terms"], Fixture::Unkeyed, "terms"),
    (
        "loft-reconcile",
        &["reconcile"],
        Fixture::Clean,
        "reconcile",
    ),
    // Init refuses any existing `spec/` before reading it.
    ("loft-init", &["init"], Fixture::Empty, "init"),
    // The skill is written whether or not there is a tree.
    (
        "loft-skill-install",
        &["skill", "install"],
        Fixture::Empty,
        "skill",
    ),
    // The driver reads the three files git hands it, never the tree, so inputs that do not
    // exist report against the path they name and write nothing.
    (
        "loft-merge-driver",
        &[
            "merge-driver",
            "base",
            "ours",
            "theirs",
            "spec/loftspec.yaml",
        ],
        Fixture::Unkeyed,
        "merge-driver",
    ),
    // The install record and the switches read and write the user's settings, never the tree.
    (
        "loft-record-install",
        &["record-install", "release-binary"],
        Fixture::Empty,
        "record-install",
    ),
    (
        "loft-telemetry",
        &["telemetry", "status"],
        Fixture::Empty,
        "telemetry",
    ),
    (
        "loft-update-check",
        &["update-check", "status"],
        Fixture::Empty,
        "update-check",
    ),
    // The yard commands act on the branch's Yard Log under `.yard/`, never on the tree; over the
    // clean fixture the branch has none until a write opens it.
    (
        "read-yard-log",
        &["yard", "read", "--session", "s1"],
        Fixture::Clean,
        "yard-read",
    ),
    (
        "write-blocks",
        &[
            "yard",
            "write",
            "--input",
            YARD_BATCH,
            "--send",
            "--session",
            "s1",
        ],
        Fixture::Clean,
        "yard-write",
    ),
    (
        "withdraw-a-submission",
        &[
            "yard",
            "withdraw",
            "--submission",
            "01a16502-f6d2-7666-adad-3403c0a92870",
            "--session",
            "s1",
        ],
        Fixture::Clean,
        "yard-withdraw",
    ),
    (
        "close-yard-log",
        &["yard", "close", "--session", "s1"],
        Fixture::Clean,
        "yard-close",
    ),
    (
        "loft-yard-answer",
        &["yard", "answer", "--input", YARD_ANSWERS, "--session", "s1"],
        Fixture::Clean,
        "yard-answer",
    ),
    // The protocol needs no repository.
    (
        "loft-yard-protocol",
        &["yard", "protocol"],
        Fixture::Empty,
        "yard-protocol",
    ),
    (
        "upgrade-open-yard-logs",
        &["yard", "upgrade"],
        Fixture::Clean,
        "yard-upgrade",
    ),
];
