//! `yardlog` performs no I/O and depends on no store and no crate of the command line.

use std::fs;
use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn sources(dir: &Path, found: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push((
                path.display().to_string(),
                fs::read_to_string(&path).unwrap(),
            ));
        }
    }
}

#[test]
fn yardlog_performs_no_io() {
    let mut found = Vec::new();
    sources(&crate_dir().join("src"), &mut found);
    assert!(found.len() > 10, "the scan found {} files", found.len());
    for (path, text) in found {
        for name in [
            "std::fs",
            "std::net",
            "std::process",
            "std::env",
            "std::io",
            "SystemTime",
        ] {
            assert!(!text.contains(name), "{path} names {name}");
        }
    }
}

#[test]
fn yardlog_depends_on_no_store_and_no_crate_of_the_command_line() {
    let manifest = fs::read_to_string(crate_dir().join("Cargo.toml")).unwrap();
    for name in [
        "loftspec-store",
        "yardlog-store",
        "loft-ports",
        "loft-cli",
        "software-plant",
    ] {
        assert!(!manifest.contains(name), "yardlog's manifest names {name}");
    }
}
