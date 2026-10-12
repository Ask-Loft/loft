//! `software-plant` is a dependency of no published crate.

use std::fs;
use std::path::Path;

const CRATES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

/// The lines of a manifest's `[dependencies]` and `[build-dependencies]` tables.
fn shipped_dependencies(manifest: &str) -> Vec<&str> {
    let mut shipped = false;
    let mut lines = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            shipped = matches!(line, "[dependencies]" | "[build-dependencies]");
        } else if shipped {
            lines.push(line);
        }
    }
    lines
}

#[test]
fn con_16_no_published_crate_depends_on_software_plant() {
    let mut published = 0;
    for entry in fs::read_dir(CRATES).unwrap() {
        let manifest = entry.unwrap().path().join("Cargo.toml");
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        if text.lines().any(|line| line.trim() == "publish = false") {
            continue;
        }
        published += 1;
        let depends = shipped_dependencies(&text)
            .iter()
            .any(|line| line.starts_with("software-plant"));
        assert!(
            !depends,
            "{} depends on software-plant",
            Path::new(&manifest).display()
        );
    }
    assert!(published >= 3, "the published crates are found");
}
