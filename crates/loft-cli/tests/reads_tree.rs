//! Reading the tree: the rules every command tagged `reads-tree` follows on a reader error, run
//! against each command whose Feature in this repository's own `spec/` lists the tag; and the
//! converse, that every command that reports a read error lists it.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::{Scratch, stderr, stdout};
use loftspec::codec;
use loftspec::node::Node;

const READING_LIST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/v1/reading-list/loft-authored-state.json"
);

const PRODUCT: &str = "products/reading-list/product.yaml";

/// The arguments that run each implemented command, by the slug of its Feature.
const COMMANDS: [(&str, &[&str]); 4] = [
    ("loft-check", &["check"]),
    ("loft-fmt", &["fmt"]),
    ("loft-render", &["render"]),
    ("loft-import-v1", &["import", "v1", READING_LIST]),
];

/// Tagged commands not implemented yet, by Feature slug and subcommand. Each must still be
/// unknown to `loft`; once it exists it moves to [`COMMANDS`] and is held to the rules.
const PENDING: [(&str, &str); 2] = [("loft-verify", "verify"), ("loft-publish", "publish")];

/// The slugs of the Features in this repository's `spec/` that list `reads-tree`.
fn tagged() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = loftspec_store::fs::read_tree(&root).unwrap();
    let document = codec::read(&files).unwrap();
    let mut slugs = Vec::new();
    document.walk(&mut |node| {
        if let Node::Feature(feature) = node
            && feature
                .details
                .tags
                .iter()
                .any(|tag| tag.as_str() == "reads-tree")
        {
            slugs.push(feature.slug.as_str().to_owned());
        }
    });
    slugs.sort();
    slugs
}

/// Every file under the scratch directory with its bytes.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(path.clone(), fs::read(&path).unwrap());
            }
        }
    }
    files
}

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
    let pending: BTreeMap<&str, &str> = PENDING.into_iter().collect();
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
    let tagged = tagged();
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
    let help = stdout(&Scratch::with_reading_list().loft(&["--help"]));
    let listed: Vec<&str> = help
        .lines()
        .skip_while(|line| !line.starts_with("Commands:"))
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help")
        .collect();
    for name in listed {
        assert!(
            COMMANDS.iter().any(|(_, args)| args[0] == name),
            "`loft {name}` has no entry in COMMANDS"
        );
    }
}
