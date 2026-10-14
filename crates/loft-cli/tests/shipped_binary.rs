//! The one binary `loft-cli` ships, and that the tests run that binary as it ships. The manifest
//! declares a single binary target, `loft`, and nothing Cargo would find as another. No crate
//! built into it declares a feature or reads the build's configuration, the tests turn on no
//! feature, the release builds it with no feature flag, and every test that starts `loft` starts
//! the binary Cargo built for this package.

use crate::common::workspace;
use std::fs;
use std::path::{Path, PathBuf};

/// The crates built into `loft`.
const BUILT_IN: [&str; 3] = ["loft-cli", "loftspec", "loftspec-store"];

/// What a test may start other than `loft`: the archiver and Node, which unpack and run the npm
/// packages built around it, and the scripts that build and smoke-test those packages.
const OTHER_PROGRAMS: [&str; 4] = [
    "\"tar\"",
    "\"node\"",
    "workspace().join(\"scripts/npm-packages.sh\")",
    "workspace().join(\"scripts/npm-smoke.sh\")",
];

/// How a test starts `loft`.
const LOFT: &str = "env!(\"CARGO_BIN_EXE_loft\")";

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn manifest(krate: &str) -> String {
    read(&workspace().join("crates").join(krate).join("Cargo.toml"))
}

/// The trimmed lines of each table `header` names in `manifest`, one list per table.
fn tables<'a>(manifest: &'a str, header: &str) -> Vec<Vec<&'a str>> {
    let mut found: Vec<Vec<&str>> = Vec::new();
    let mut inside = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == header;
            if inside {
                found.push(Vec::new());
            }
        } else if inside && !line.is_empty() && !line.starts_with('#') {
            found.last_mut().unwrap().push(line);
        }
    }
    found
}

/// Every Rust source file under `dir`.
fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

fn shown(path: &Path) -> String {
    path.strip_prefix(workspace())
        .unwrap()
        .display()
        .to_string()
}

/// The argument of each `Command::new(…)` in `text`, as written.
fn programs(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (start, _) in text.match_indices("Command::new(") {
        let rest = &text[start + "Command::new(".len()..];
        let mut depth = 0;
        let end = rest
            .char_indices()
            .find(|&(_, c)| {
                depth += match c {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                };
                depth < 0
            })
            .map_or(rest.len(), |(end, _)| end);
        found.push(rest[..end].trim().to_owned());
    }
    found
}

#[test]
fn the_package_exposes_exactly_one_binary_named_loft() {
    let manifest = manifest("loft-cli");
    let bins = tables(&manifest, "[[bin]]");
    assert_eq!(
        bins.len(),
        1,
        "crates/loft-cli/Cargo.toml declares {bins:?}"
    );
    assert!(bins[0].contains(&"name = \"loft\""), "{bins:?}");
    assert!(bins[0].contains(&"path = \"src/main.rs\""), "{bins:?}");
    let package = &tables(&manifest, "[package]")[0];
    assert!(
        !package.contains(&"autobins = true"),
        "crates/loft-cli/Cargo.toml asks Cargo to find binaries"
    );
    let found = workspace().join("crates/loft-cli/src/bin");
    assert!(!found.exists(), "{} holds more binaries", shown(&found));
}

#[test]
fn the_tests_run_the_loft_binary_as_it_ships() {
    for krate in BUILT_IN {
        let manifest = manifest(krate);
        assert_eq!(
            tables(&manifest, "[features]"),
            Vec::<Vec<&str>>::new(),
            "{krate}"
        );
        let mut files = Vec::new();
        sources(
            &workspace().join("crates").join(krate).join("src"),
            &mut files,
        );
        assert!(
            files.len() > 5,
            "{krate}: the scan found {} files",
            files.len()
        );
        for file in files {
            let text = read(&file);
            for token in ["cfg(feature", "cfg!(feature", "debug_assertions"] {
                assert!(!text.contains(token), "{} reads `{token}`", shown(&file));
            }
        }
    }
    for (krate, header) in [
        ("loft-cli", "[dev-dependencies]"),
        ("software-plant", "[dependencies]"),
    ] {
        for line in tables(&manifest(krate), header).concat() {
            assert!(!line.contains("features"), "{krate}: {line}");
        }
    }
    let release = read(&workspace().join(".github/workflows/release.yml"));
    let builds: Vec<&str> = release
        .lines()
        .filter(|line| line.contains("cargo build") && line.contains("-p loft-cli"))
        .collect();
    assert_eq!(builds.len(), 1, "release.yml builds loft {builds:?}");
    assert!(!builds[0].contains("features"), "{}", builds[0]);
    assert_the_tests_start_only_the_built_loft();
}

/// Every program a test of this crate starts is the `loft` Cargo built or one of
/// [`OTHER_PROGRAMS`].
fn assert_the_tests_start_only_the_built_loft() {
    let mut files = Vec::new();
    sources(&workspace().join("crates/loft-cli/tests"), &mut files);
    let mut started = 0;
    for file in files
        .iter()
        .filter(|file| !file.ends_with("shipped_binary.rs"))
    {
        for program in programs(&read(file)) {
            started += usize::from(program == LOFT);
            assert!(
                program == LOFT || OTHER_PROGRAMS.contains(&program.as_str()),
                "{} starts {program}",
                shown(file)
            );
        }
    }
    assert!(started >= 5, "the scan found {started} starts of loft");
}
