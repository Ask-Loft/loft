//! `loft-common` holds what the formats share and reaches outside no process: it performs no I/O
//! and depends on no other crate of the workspace.

use std::fs;
use std::path::{Path, PathBuf};

fn crate_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The names a manifest's `table` lists, each line's name before its `=` or `.`.
fn listed(manifest: &str, table: &str) -> Vec<String> {
    manifest
        .lines()
        .skip_while(|line| line.trim() != table)
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split(['=', '.']).next())
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// The workspace's crates, by the last part of each member's path.
fn workspace_crates() -> Vec<String> {
    let root = fs::read_to_string(crate_dir().join("../../Cargo.toml")).unwrap();
    root.lines()
        .skip_while(|line| !line.starts_with("members"))
        .skip(1)
        .take_while(|line| !line.starts_with(']'))
        .filter_map(|line| line.trim().trim_matches([',', '"']).rsplit('/').next())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

fn sources(dir: &Path, found: &mut Vec<(PathBuf, String)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else {
            let text = fs::read_to_string(&path).unwrap();
            found.push((path, text));
        }
    }
}

#[test]
fn the_common_crate_depends_on_no_other_crate_of_the_workspace() {
    let members = workspace_crates();
    assert!(members.len() > 6, "the scan found {members:?}");
    assert!(members.iter().any(|name| name == "loftspec"), "{members:?}");
    let manifest = fs::read_to_string(crate_dir().join("Cargo.toml")).unwrap();
    let dependencies = listed(&manifest, "[dependencies]");
    assert!(
        dependencies.contains(&"uuid".to_owned()),
        "{dependencies:?}"
    );
    for name in &dependencies {
        assert!(!members.contains(name), "loft-common depends on {name}");
    }
}

#[test]
fn no_source_of_the_common_crate_reaches_outside_the_process() {
    let mut found = Vec::new();
    sources(&crate_dir().join("src"), &mut found);
    assert!(found.len() > 6, "the scan found {} files", found.len());
    for (path, text) in found {
        for reach in ["std::fs", "std::net", "std::process", "std::env", "std::io"] {
            assert!(!text.contains(reach), "{} names {reach}", path.display());
        }
    }
}
