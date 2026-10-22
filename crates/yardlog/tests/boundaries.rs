//! `yardlog` performs no I/O, depends on no store and no crate of the command line, and depends
//! not on `loftspec` but on `loft-common`, which the Format shares.

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

#[test]
fn yardlog_does_not_depend_on_the_format_s_crate() {
    let manifest = fs::read_to_string(crate_dir().join("Cargo.toml")).unwrap();
    let dependencies: Vec<&str> = manifest
        .lines()
        .skip_while(|line| line.trim() != "[dependencies]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split(['=', '.']).next())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    assert!(dependencies.contains(&"loft-common"), "{dependencies:?}");
    assert!(!dependencies.contains(&"loftspec"), "{dependencies:?}");
    let mut found = Vec::new();
    sources(&crate_dir().join("src"), &mut found);
    assert!(found.len() > 10, "the scan found {} files", found.len());
    for (path, text) in found {
        assert!(!text.contains("loftspec"), "{path} names loftspec");
    }
}
