//! What the `loftspec` package promises of itself, held by guards over its source, its manifest
//! and CI: it performs no file or network input or output, its renderer reads no clock, locale or
//! environment variable, it declares the Apache-2.0 licence, and CI builds it with the toolchain
//! its declared `rust-version` names.

use std::fs;
use std::path::{Path, PathBuf};

const CRATE: &str = env!("CARGO_MANIFEST_DIR");

/// What only input or output, a process or the environment uses: a module, a type or a call.
/// `std::io` itself is allowed, since the hasher writes into a digest through `io::Write`, and so
/// is `std::path`, since the reconciler inspects the components of a path it is given.
const IO: [&str; 28] = [
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "fs::",
    "net::",
    "process::",
    "env::",
    "Command::new",
    "File::open",
    "File::create",
    "OpenOptions",
    "TcpStream",
    "UdpSocket",
    "stdin(",
    "stdout(",
    "stderr(",
    "print!",
    "println!",
    "dbg!",
    ".exists(",
    ".try_exists(",
    ".is_file(",
    ".is_dir(",
    ".metadata(",
    ".read_dir(",
    ".read_link(",
    ".canonicalize(",
];

/// What reads a clock, a locale or the environment. `Key::mint` reads the clock, so the renderer
/// and the hasher mint no key.
const CLOCK_LOCALE_ENVIRONMENT: [&str; 14] = [
    "SystemTime",
    "Instant",
    "std::time",
    "chrono",
    "now_v7",
    "mint(",
    "std::env",
    "env::",
    "env!(",
    "LANG",
    "LC_",
    "\"TZ\"",
    "locale",
    "Locale",
];

/// The dependencies `loftspec` ships with, none of which performs input or output for it. A new
/// one is admitted here only once it is known to perform none.
const DEPENDENCIES: [&str; 9] = [
    "pulldown-cmark",
    "serde",
    "serde_json",
    "serde_json_canonicalizer",
    "sha2",
    "thiserror",
    "unicode-properties",
    "unicode-segmentation",
    "uuid",
];

fn workspace() -> PathBuf {
    Path::new(CRATE).join("../..")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
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

/// How many source files lie under each of `dirs` of `src/`, and each use of one of `tokens` in
/// their code, outside comments, as `<file>:<line>: <token>`.
fn uses(dirs: &[&str], tokens: &[&str]) -> (usize, Vec<String>) {
    let src = Path::new(CRATE).join("src");
    let mut files = Vec::new();
    for dir in dirs {
        sources(&src.join(dir), &mut files);
    }
    let mut found = Vec::new();
    for file in &files {
        let shown = file.strip_prefix(&src).unwrap().display().to_string();
        for (number, line) in read(file).lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for token in tokens.iter().filter(|token| line.contains(*token)) {
                found.push(format!("src/{shown}:{}: {token}", number + 1));
            }
        }
    }
    (files.len(), found)
}

/// The trimmed lines of the table `header` names in `manifest`.
fn table<'a>(manifest: &'a str, header: &str) -> Vec<&'a str> {
    let mut inside = false;
    let mut lines = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == header;
        } else if inside && !line.is_empty() && !line.starts_with('#') {
            lines.push(line);
        }
    }
    lines
}

/// The names of the dependencies a manifest ships with: those of its `[dependencies]` and
/// `[build-dependencies]` tables, for every target.
fn shipped_dependencies(manifest: &str) -> Vec<String> {
    let mut shipped = false;
    let mut names = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if let Some(header) = line.strip_prefix('[') {
            let header = header.trim_matches(['[', ']']);
            if let Some(name) = header.strip_prefix("dependencies.") {
                names.push(name.to_owned());
            }
            shipped = ["dependencies", "build-dependencies"]
                .iter()
                .any(|table| header == *table || header.ends_with(&format!(".{table}")));
        } else if shipped && !line.is_empty() && !line.starts_with('#') {
            names.push(line.split(['.', '=', ' ']).next().unwrap().to_owned());
        }
    }
    names
}

/// The string `key = "<value>"` gives among `lines`.
fn string_value(lines: &[&str], key: &str) -> Option<String> {
    lines.iter().find_map(|line| {
        let value = line.strip_prefix(key)?.trim_start().strip_prefix('=')?;
        Some(
            value
                .trim()
                .strip_prefix('"')?
                .strip_suffix('"')?
                .to_owned(),
        )
    })
}

/// The value of `key` in the package's `[package]` table, or in the workspace's
/// `[workspace.package]` table where the package inherits it.
fn package_value(key: &str) -> String {
    let manifest = read(&Path::new(CRATE).join("Cargo.toml"));
    let package = table(&manifest, "[package]");
    if let Some(value) = string_value(&package, key) {
        return value;
    }
    let inherited = format!("{key}.workspace = true");
    assert!(
        package.contains(&inherited.as_str()),
        "crates/loftspec/Cargo.toml neither declares nor inherits `{key}`"
    );
    let root = read(&workspace().join("Cargo.toml"));
    string_value(&table(&root, "[workspace.package]"), key)
        .unwrap_or_else(|| panic!("Cargo.toml: `[workspace.package]` declares no `{key}`"))
}

/// The trimmed lines of the job `name` in a workflow.
fn job<'a>(workflow: &'a str, name: &str) -> Vec<&'a str> {
    let start = format!("  {name}:");
    workflow
        .lines()
        .skip_while(|line| *line != start)
        .skip(1)
        .take_while(|line| line.is_empty() || line.starts_with("   "))
        .map(str::trim)
        .collect()
}

#[test]
fn the_library_uses_no_file_network_process_or_environment() {
    let (scanned, found) = uses(&[""], &IO);
    assert!(scanned > 50, "the scan found {scanned} source files");
    assert_eq!(found, Vec::<String>::new());
    let manifest = read(&Path::new(CRATE).join("Cargo.toml"));
    assert_eq!(
        shipped_dependencies(&manifest),
        DEPENDENCIES,
        "crates/loftspec/Cargo.toml: a new dependency must be known to perform no input or output"
    );
}

#[test]
fn the_renderer_and_the_hasher_read_no_clock_locale_or_environment() {
    let (scanned, found) = uses(&["render", "hash"], &CLOCK_LOCALE_ENVIRONMENT);
    assert!(scanned >= 7, "the scan found {scanned} source files");
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn the_package_declares_the_apache_licence() {
    assert_eq!(package_value("license"), "Apache-2.0");
    let manifest = read(&Path::new(CRATE).join("Cargo.toml"));
    let package = table(&manifest, "[package]");
    assert!(
        !package.iter().any(|line| line.starts_with("license-file")),
        "crates/loftspec/Cargo.toml names a licence file beside the licence"
    );
}

#[test]
fn ci_builds_the_library_with_the_toolchain_its_rust_version_names() {
    let version = package_value("rust-version");
    assert!(
        version.split('.').all(|part| part.parse::<u32>().is_ok()),
        "`{version}` names no stable release"
    );
    let workflow = read(&workspace().join(".github/workflows/ci.yml"));
    let steps = job(&workflow, "rust-version");
    assert!(!steps.is_empty(), "ci.yml has no `rust-version` job");
    for step in [
        format!("- run: rustup toolchain install {version} --profile minimal"),
        format!("- run: cargo +{version} build -p loftspec --locked"),
    ] {
        assert!(
            steps.contains(&step.as_str()),
            "ci.yml: the `rust-version` job lacks `{step}`"
        );
    }
}
