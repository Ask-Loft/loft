//! The one binary `loft-cli` ships, and that the tests run that binary as it ships. The manifest
//! declares a single binary target, `loft`, and nothing Cargo would find as another. No crate
//! built into it declares a feature or reads the build's configuration, the tests turn on no
//! feature, the release's build builds it with no feature flag, and every test that starts `loft` starts
//! the binary Cargo built for this package.

use crate::common::{shown, sources, workspace};
use crate::workflow::read;
use std::path::Path;

/// The crates built into `loft`.
const BUILT_IN: [&str; 3] = ["loft-cli", "loftspec", "loftspec-store"];

/// What a test may start other than `loft`: the archiver, Node and npm, which unpack, run and
/// install the npm packages built around it, the scripts that build and smoke-test those
/// packages, the release check, copied into a scratch repository, the built `loft` itself placed
/// where an npm install puts it, which only `Scratch::npm_installed` starts, to show how `loft`
/// tells it was installed by npm; `whoami` and `hostname`, whose answers a test plants as text the command
/// event must never carry; `script`, which runs the built `loft` on a pseudo-terminal; and
/// `mkfifo`, which makes the named pipe that a test file path names.
const OTHER_PROGRAMS: [&str; 11] = [
    "\"mkfifo\"",
    "&copy",
    "\"script\"",
    "\"whoami\"",
    "\"hostname\"",
    "\"tar\"",
    "\"node\"",
    "\"npm\"",
    "workspace().join(\"scripts/npm-packages.sh\")",
    "workspace().join(\"scripts/npm-smoke.sh\")",
    "repo.join(SCRIPT)",
];

/// How a test starts `loft`.
const LOFT: &str = "env!(\"CARGO_BIN_EXE_loft\")";

fn manifest(krate: &str) -> String {
    read(Path::new("crates").join(krate).join("Cargo.toml"))
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
        let files = sources(&workspace().join("crates").join(krate).join("src"));
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
    let build = read(".github/workflows/build.yml");
    let builds: Vec<&str> = build
        .lines()
        .filter(|line| line.contains("cargo build") && line.contains("-p loft-cli"))
        .collect();
    assert_eq!(builds.len(), 1, "build.yml builds loft {builds:?}");
    assert!(!builds[0].contains("features"), "{}", builds[0]);
    assert_the_tests_start_only_the_built_loft();
}

/// Every program a test of this crate starts is the `loft` Cargo built or one of
/// [`OTHER_PROGRAMS`].
fn assert_the_tests_start_only_the_built_loft() {
    let files = sources(&workspace().join("crates/loft-cli/tests"));
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
