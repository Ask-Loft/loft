//! What the release workflow, `.github/workflows/release.yml`, promises, held by guards over its
//! jobs: a version tag starts it, the release check, the gate and `loft verify` run on the tag's
//! commit before anything is built, the npm packages are packed and run before the approval and
//! run again before anything is published, the GitHub release carries every platform's binary,
//! and no registry token is stored in the repository.

use std::fs;
use std::path::Path;

use crate::common::{stdout, workspace};
use software_plant::real_git::real_git;

fn read(path: &str) -> String {
    let path = workspace().join(path);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn release() -> String {
    read(".github/workflows/release.yml")
}

/// The trimmed lines of the job `name` in a workflow.
fn job<'a>(workflow: &'a str, name: &str) -> Vec<&'a str> {
    let start = format!("  {name}:");
    let lines: Vec<&str> = workflow
        .lines()
        .skip_while(|line| *line != start)
        .skip(1)
        .take_while(|line| line.is_empty() || line.starts_with("   "))
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert!(!lines.is_empty(), "the workflow has no job `{name}`");
    lines
}

/// Where `line` stands among the lines of the job `name`.
fn position(lines: &[&str], line: &str, name: &str) -> usize {
    lines
        .iter()
        .position(|each| *each == line)
        .unwrap_or_else(|| panic!("the job `{name}` lacks `{line}`"))
}

/// The six supported platforms, by Node's names: macOS, Linux and Windows, each on x64 and arm64.
fn supported() -> Vec<String> {
    let mut platforms = Vec::new();
    for os in ["darwin", "linux", "win32"] {
        for cpu in ["arm64", "x64"] {
            platforms.push(format!("{os}-{cpu}"));
        }
    }
    platforms
}

/// Each entry of the build job's matrix, as its platform and its Rust target.
fn matrix(workflow: &str) -> Vec<(String, String)> {
    let field = |line: &str, name: &str| {
        let (_, rest) = line
            .split_once(&format!("{name}: "))
            .unwrap_or_else(|| panic!("no {name} in `{line}`"));
        rest.split([',', ' ', '}']).next().unwrap().to_owned()
    };
    job(workflow, "build")
        .into_iter()
        .filter(|line| line.starts_with("- { platform: "))
        .map(|line| (field(line, "platform"), field(line, "target")))
        .collect()
}

#[test]
fn a_version_tag_runs_the_release_check_on_the_tag_s_commit_first() {
    let workflow = release();
    let trigger: Vec<&str> = workflow
        .lines()
        .skip_while(|line| *line != "on:")
        .skip(1)
        .take_while(|line| line.starts_with(' ') || line.is_empty())
        .filter(|line| !line.is_empty())
        .map(str::trim)
        .collect();
    assert_eq!(trigger, ["push:", "tags: [\"v*\"]"]);
    let check = job(&workflow, "check");
    // The whole history, so that the check can find the tag's commit on `origin/main`.
    let checkout = position(&check, "- uses: actions/checkout@v7", "check");
    assert_eq!(
        check[checkout + 1..checkout + 3],
        ["with:", "fetch-depth: 0"]
    );
    assert!(
        !check.iter().any(|line| line.starts_with("ref:")),
        "{check:?}"
    );
    let release_check = position(
        &check,
        "- run: scripts/release-check.sh \"$GITHUB_REF_NAME\"",
        "check",
    );
    let first_run = check
        .iter()
        .position(|line| line.starts_with("- run: cargo"));
    assert!(
        first_run.is_some_and(|first| release_check < first),
        "{check:?}"
    );
}

#[test]
fn the_check_job_runs_the_gate_and_verify_and_the_build_waits_for_it() {
    let workflow = release();
    let check = job(&workflow, "check");
    for step in [
        "- run: cargo fmt --all -- --check",
        "- run: cargo clippy --workspace --all-targets -- -D warnings",
        "- run: cargo nextest run --workspace",
        "- run: cargo run -q -p loft-cli -- fmt --check",
        "- run: cargo run -q -p loft-cli -- check --fail-on warning",
        "- run: cargo run -q -p loft-cli -- reconcile --fail-on warning",
        "- run: cargo run -q -p loft-cli -- verify",
    ] {
        position(&check, step, "check");
    }
    for (name, needs) in [
        ("build", "check"),
        ("package", "build"),
        ("publish", "package"),
    ] {
        let lines = job(&workflow, name);
        assert_eq!(lines[0], format!("needs: {needs}"), "the job `{name}`");
    }
}

#[test]
fn the_github_release_carries_an_archive_of_each_platform_s_binary() {
    let workflow = release();
    let platforms: Vec<String> = matrix(&workflow).into_iter().map(|(p, _)| p).collect();
    assert_eq!(platforms.len(), 6, "{platforms:?}");
    let build = job(&workflow, "build");
    for line in [
        "mkdir -p \"binaries/${{ matrix.platform }}\"",
        "cp \"target/${{ matrix.target }}/release/$executable\" \"binaries/${{ matrix.platform }}/\"",
        "name: binary-${{ matrix.platform }}",
        "path: binaries",
    ] {
        position(&build, line, "build");
    }
    let package = job(&workflow, "package");
    for line in [
        "pattern: binary-*",
        "merge-multiple: true",
        "for dir in binaries/*/; do",
        "(cd \"$dir\" && zip -q \"$OLDPWD/release/archives/loft-$version-$platform.zip\" loft.exe)",
        "tar -czf \"release/archives/loft-$version-$platform.tar.gz\" -C \"$dir\" loft",
        "name: release",
        "path: release",
    ] {
        position(&package, line, "package");
    }
    let publish = job(&workflow, "publish");
    position(
        &publish,
        "gh release upload \"$GITHUB_REF_NAME\" release/archives/* --clobber",
        "publish",
    );
}

#[test]
fn the_build_the_npm_packages_and_their_tests_name_the_same_six_platforms() {
    let supported = supported();
    let mut built = Vec::new();
    for (platform, target) in matrix(&release()) {
        let (os, cpu) = platform.split_once('-').unwrap();
        let system = match os {
            "darwin" => "apple-darwin",
            "linux" => "unknown-linux-musl",
            "win32" => "pc-windows-msvc",
            other => panic!("{other}"),
        };
        let arch = if cpu == "x64" { "x86_64" } else { "aarch64" };
        assert_eq!(target, format!("{arch}-{system}"), "{platform}");
        built.push(platform);
    }
    built.sort();
    assert_eq!(built, supported);
    let script = read("scripts/npm-packages.sh");
    let listed = script
        .lines()
        .find_map(|line| line.strip_prefix("platforms=\""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("scripts/npm-packages.sh lists its platforms");
    let mut packed: Vec<&str> = listed.split(' ').collect();
    packed.sort_unstable();
    assert_eq!(packed, supported);
    let mut tested = crate::npm::PLATFORMS.to_vec();
    tested.sort_unstable();
    assert_eq!(tested, supported);
}

/// Asserts the job `package` runs `step` and the approval comes after it: the job has no
/// environment, and the publishing job, which needs it, runs in the one that requires approval.
fn assert_before_the_approval(workflow: &str, step: &str) -> usize {
    let package = job(workflow, "package");
    assert!(
        !package.iter().any(|line| line.starts_with("environment:")),
        "{package:?}"
    );
    let publish = job(workflow, "publish");
    position(&publish, "needs: package", "publish");
    position(&publish, "environment: release", "publish");
    for name in ["check", "build"] {
        let lines = job(workflow, name);
        assert!(
            !lines.iter().any(|line| line.starts_with("environment:")),
            "{name}"
        );
    }
    position(&package, step, "package")
}

#[test]
fn each_npm_package_is_packed_into_a_tarball_before_the_approval() {
    let workflow = release();
    let packed = assert_before_the_approval(
        &workflow,
        "- run: scripts/npm-packages.sh binaries release/npm",
    );
    let package = job(&workflow, "package");
    let uploaded = position(&package, "path: release", "package");
    assert!(packed < uploaded, "{package:?}");
}

#[test]
fn loft_runs_from_the_tarballs_before_the_approval() {
    let workflow = release();
    let packed = assert_before_the_approval(
        &workflow,
        "- run: scripts/npm-packages.sh binaries release/npm",
    );
    let smoke = assert_before_the_approval(&workflow, "- run: scripts/npm-smoke.sh release/npm");
    assert!(packed < smoke);
}

#[test]
fn the_publishing_job_runs_the_tarballs_it_downloaded_before_it_publishes() {
    let workflow = release();
    let publish = job(&workflow, "publish");
    let download = position(&publish, "- uses: actions/download-artifact@v8", "publish");
    assert_eq!(
        publish[download + 1..download + 4],
        ["with:", "name: release", "path: release"]
    );
    let smoke = position(
        &publish,
        "- run: scripts/npm-smoke.sh release/npm",
        "publish",
    );
    let crates = position(&publish, "- run: scripts/publish-crates.sh", "publish");
    let npm = position(
        &publish,
        "- run: scripts/publish-npm.sh release/npm",
        "publish",
    );
    let github = position(&publish, "- name: GitHub release", "publish");
    assert!(
        download < smoke && smoke < crates.min(npm).min(github),
        "{publish:?}"
    );
    assert!(
        !publish.iter().any(|line| line.contains("npm-packages.sh")),
        "the publishing job packs the packages anew"
    );
}

/// Whether `text` holds what a registry token looks like: an npm token, `npm_` and 36 letters
/// and digits; a crates.io token, `cio` and 32; or an `.npmrc` entry that carries a token.
fn holds_a_token(text: &str) -> bool {
    let run = |at: usize| {
        text[at..]
            .bytes()
            .take_while(u8::is_ascii_alphanumeric)
            .count()
    };
    let prefixed = |prefix: &str, length: usize| {
        text.match_indices(prefix)
            .any(|(at, _)| run(at + prefix.len()) >= length)
    };
    prefixed("npm_", 36) || prefixed("cio", 32) || text.contains(concat!("_auth", "Token"))
}

#[test]
fn a_token_in_a_file_is_found() {
    let npm = format!("npm_{}", "a1".repeat(18));
    let crates_io = format!("cio{}", "B2".repeat(16));
    let npmrc = format!("//registry.npmjs.org/:{}=x", concat!("_auth", "Token"));
    for text in [npm, crates_io, npmrc] {
        assert!(holds_a_token(&text), "{text}");
    }
    assert!(!holds_a_token("npm_config_cache precious cio"));
}

#[test]
fn the_release_workflow_stores_no_registry_token_in_the_repository() {
    for workflow in ["ci.yml", "release.yml"] {
        let text = read(&format!(".github/workflows/{workflow}"));
        for line in text.lines().map(str::trim) {
            let Some((key, value)) = line.split_once(": ") else {
                continue;
            };
            if line.starts_with('#') || !key.to_ascii_lowercase().contains("token") {
                continue;
            }
            let given = value.starts_with("${{ secrets.")
                || ["${{ steps.auth.outputs.token }}", "${{ github.token }}"].contains(&value)
                || (key == "id-token" && value == "write");
            assert!(given, "{workflow}: `{line}`");
        }
    }
    let listed = real_git()
        .arg("-C")
        .arg(workspace())
        .args(["ls-files", "-z"])
        .output()
        .unwrap();
    assert!(listed.status.success());
    let files: Vec<String> = stdout(&listed)
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect();
    assert!(files.len() > 100, "git lists {} files", files.len());
    for file in files {
        let path = workspace().join(&file);
        assert!(
            !Path::new(&file).ends_with(".npmrc"),
            "{file}: an npm configuration in the repository"
        );
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        assert!(!holds_a_token(&text), "{file} holds a registry token");
    }
}
