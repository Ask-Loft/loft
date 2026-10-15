//! The files table of the format reference against the files `loft` reads and writes: it names
//! the file of each role the reader gives a path, the files beside `spec/`, and each command
//! that writes each file, as `loft init` and `loft publish` are seen to write them.

use std::collections::{BTreeMap, BTreeSet};

use crate::common::BESIDE;
use crate::common::{EPOCH, Scratch, stderr};
use crate::markdown::{rows_by_span, section, table};
use crate::skill_format::format;
use crate::snapshot::{self, snapshot};
use loftspec::codec::{Role, SpecPath};
use loftspec::publish::LOCK;
use loftspec::trace::TRACE;
use loftspec_store::fs::SIDECAR;
use software_plant::{Clock, GitScenario};

/// A file of each role, its names standing for any: an exhaustive match, so a new role fails
/// to compile here until the table names its file.
fn placeholder(role: &Role) -> Role {
    let any = |name: &str| format!("<{name}>");
    match role {
        Role::Manifest | Role::Labels | Role::Tags | Role::Categories | Role::Lint => role.clone(),
        Role::Product { .. } => Role::Product {
            product: any("product"),
        },
        Role::Feature { .. } => Role::Feature {
            product: any("product"),
            group: any("group"),
            feature: any("feature"),
        },
        Role::Component { .. } => Role::Component {
            slug: any("component"),
        },
        Role::Decision { .. } => Role::Decision {
            slug: any("decision"),
        },
    }
}

fn roles() -> Vec<Role> {
    let slug = String::new;
    let roles = [
        Role::Manifest,
        Role::Labels,
        Role::Tags,
        Role::Categories,
        Role::Lint,
        Role::Product { product: slug() },
        Role::Feature {
            product: slug(),
            group: slug(),
            feature: slug(),
        },
        Role::Component { slug: slug() },
        Role::Decision { slug: slug() },
    ];
    roles.iter().map(placeholder).collect()
}

/// A path from the directory that holds `spec/`, as the files table names it.
fn pattern(path: &str) -> String {
    path.strip_prefix("spec/").map_or_else(
        || path.to_owned(),
        |inner| {
            let role = Role::of(&SpecPath::new(inner)).unwrap_or_else(|| panic!("{path}"));
            placeholder(&role).path().to_string()
        },
    )
}

/// The files table's rows, by the path each names.
fn files_table(markdown: &str) -> BTreeMap<String, Vec<String>> {
    let table = table(section(markdown, "The files of `spec/`"), "Path");
    let rows = rows_by_span(&table);
    rows.into_iter()
        .map(|(path, row)| (path.to_owned(), row.to_vec()))
        .collect()
}

#[test]
fn the_files_table_names_the_file_of_each_kind_of_record() {
    let named: BTreeSet<String> = files_table(&format()).into_keys().collect();
    let mut files: BTreeSet<String> = roles().iter().map(|role| role.path().to_string()).collect();
    files.extend(BESIDE.map(str::to_owned));
    assert_eq!(named, files);
}

/// The files that differ between two snapshots, as the files table names them.
fn changed(before: &BTreeMap<String, Vec<u8>>, after: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    snapshot::changed(before, after)
        .iter()
        .map(|path| pattern(path))
        .collect()
}

/// The files `loft init --no-skill` writes, as the files table names them.
fn initialised() -> Vec<String> {
    let scratch = Scratch::empty();
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&["init", "--no-skill"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    changed(&before, &snapshot(&scratch.0))
}

/// The files a first publish of the Reading List writes, as the files table names them.
fn published() -> Vec<String> {
    let scratch = Scratch::with_reading_list();
    let before = snapshot(&scratch.0);
    let git = GitScenario::clean_on("main");
    let output = scratch.loft_with(&["publish"], Some(&git), &Clock::fixed(EPOCH));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    changed(&before, &snapshot(&scratch.0))
}

#[test]
fn the_files_table_names_each_command_that_writes_a_file() {
    let markdown = format();
    let rows = files_table(&markdown);
    let says = |path: &str, command: &str| {
        let row = rows
            .get(path)
            .unwrap_or_else(|| panic!("no row names {path}"));
        let written = row[2].contains(&format!("`{command}`"));
        assert!(written, "{path} is written by {command}: {}", row[2]);
    };
    let initialised = initialised();
    assert!(initialised.len() >= 4, "{initialised:?}");
    for path in initialised {
        says(&path, "loft init");
    }
    for role in roles().iter().filter(|role| role.is_written()) {
        says(&role.path().to_string(), "loft fmt");
    }
    let published = published();
    assert!(published.contains(&LOCK.to_owned()), "{published:?}");
    for path in published {
        says(&path, "loft publish");
    }
    says(TRACE, "loft reconcile --accept");
    says(SIDECAR, "loft import v1");
    for command in ["loft import v1", "loft skill install"] {
        assert!(
            markdown.contains(&format!("`{command}` writes")),
            "{command}"
        );
    }
}
