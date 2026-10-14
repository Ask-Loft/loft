//! Every command `loft` implements, once: the slug of its Feature in this repository's own
//! `spec/`, the arguments that run it, and the fixture it runs over. The tests that hold every
//! command to a rule read it; the first argument is the subcommand, which names its schema.

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

/// Each command by the slug of its Feature, with the arguments that run it and its fixture.
pub const COMMANDS: [(&str, &[&str], Fixture); 11] = [
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
];
