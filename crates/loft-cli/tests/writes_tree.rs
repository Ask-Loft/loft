//! Writing the tree: every command tagged `writes-tree` finishes a committed save it finds and
//! discards an uncommitted one before it acts, run against each command whose Feature in this
//! repository's own `spec/` lists the tag; and the converse, that every command that changes a
//! file under `spec/` or beside it lists the tag.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use common::{Scratch, stderr, stdout};
use loftspec::codec::SpecPath;
use loftspec::node::Node;
use loftspec_store::journal::{self, Changes, JOURNAL, RootPath};
use loftspec_store::{DocumentStore, FileStore};

const READING_LIST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/v1/reading-list/loft-authored-state.json"
);

const PRODUCT: &str = "products/reading-list/product.yaml";

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

/// The slugs of the Features in this repository's `spec/` that list `writes-tree`.
fn tagged() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let document = FileStore::open(&root).unwrap().load().unwrap();
    let mut slugs = Vec::new();
    document.walk(&mut |node| {
        if let Node::Feature(feature) = node
            && feature
                .details
                .tags
                .iter()
                .any(|tag| tag.as_str() == "writes-tree")
        {
            slugs.push(feature.slug.as_str().to_owned());
        }
    });
    slugs.sort();
    slugs
}

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
    let mut files = BTreeMap::new();
    let mut pending = vec![root.join("spec")];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let name = path.strip_prefix(root).unwrap().display().to_string();
                files.insert(name, fs::read(&path).unwrap());
            }
        }
    }
    for beside in ["v1-sidecar.json", "loft.lock.json"] {
        if let Ok(bytes) = fs::read(root.join(beside)) {
            files.insert(beside.to_owned(), bytes);
        }
    }
    files
}

/// The implemented commands whose Features list the tag, after checking that each pending one
/// is still unknown to `loft`.
fn implemented_tagged() -> Vec<(&'static str, &'static [&'static str], bool)> {
    let pending: BTreeMap<&str, &str> = PENDING.into_iter().collect();
    let mut commands = Vec::new();
    for slug in tagged() {
        if let Some(subcommand) = pending.get(slug.as_str()) {
            let output = Scratch::with_reading_list().loft(&[subcommand]);
            assert_eq!(
                output.status.code(),
                Some(2),
                "`loft {subcommand}` exists now: move {slug} from PENDING to COMMANDS"
            );
            continue;
        }
        let command = COMMANDS
            .into_iter()
            .find(|(name, _, _)| *name == slug)
            .unwrap_or_else(|| panic!("{slug} lists `writes-tree` but COMMANDS has no entry"));
        commands.push(command);
    }
    commands
}

#[test]
fn each_tagged_command_finishes_a_committed_save_before_it_acts() {
    for (slug, args, empty) in implemented_tagged() {
        let scratch = fixture(empty);
        plant(&scratch, empty, true);
        let output = scratch.loft(args);
        // In an empty directory the finished save leaves `spec/` not empty, which the command
        // must see before it writes.
        assert_eq!(
            output.status.code(),
            Some(i32::from(empty)),
            "{slug}: the exit code\n{}{}",
            stdout(&output),
            stderr(&output)
        );
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
        let output = scratch.loft(args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{slug}: the exit code\n{}{}",
            stdout(&output),
            stderr(&output)
        );
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
    let tagged = tagged();
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
        let output = scratch.loft(args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{slug}: the exit code\n{}{}",
            stdout(&output),
            stderr(&output)
        );
        assert_ne!(format_files(&scratch.0), before, "{slug}: wrote nothing");
    }
}
