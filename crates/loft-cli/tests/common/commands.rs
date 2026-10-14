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

/// Each command by the slug of its Feature, with the arguments that run it and its fixture.
pub const COMMANDS: [(&str, &[&str], Fixture); 13] = [
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
    ("loft-find", &["find", "shelf"], Fixture::Clean),
    ("loft-reconcile", &["reconcile"], Fixture::Clean),
    // Init refuses any existing `spec/` before reading it.
    ("loft-init", &["init"], Fixture::Empty),
    // The skill is written whether or not there is a tree.
    ("loft-skill-install", &["skill", "install"], Fixture::Empty),
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
    ),
    // The switches read and write the user's settings, never the tree.
    ("loft-telemetry", &["telemetry", "status"], Fixture::Empty),
    (
        "loft-update-check",
        &["update-check", "status"],
        Fixture::Empty,
    ),
];
