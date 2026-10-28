//! What the release workflow, `.github/workflows/release.yml`, promises, held by guards over its
//! jobs: a version tag starts it, the release check and the gate run on the tag's commit before
//! anything is built, and `loft verify` too for a release, the build is
//! `.github/workflows/build.yml`, which also runs by hand before a tag, the npm packages are
//! packed and run before the approval and run again before anything is published, under the
//! channel the release check names, the GitHub release carries every platform's binary and marks
//! a prerelease as one, and no registry token is stored in the repository.

use std::fs;
use std::path::Path;

use crate::common::{PLATFORMS, stdout, workspace};
use crate::workflow::{Job, block, job, read};
use software_plant::real_git::real_git;

fn release() -> String {
    read(".github/workflows/release.yml")
}

/// The build workflow, which the release calls and which also runs by hand.
fn build() -> String {
    read(".github/workflows/build.yml")
}

/// What the release's check job runs in place of a step of CI's gate, or nothing where it does
/// not run that step, with the reason.
const RELEASE_FORM: [(&str, Option<&str>); 2] = [
    // The same dry run, leaving out a crate whose version crates.io already holds, so that a
    // release that stopped part way can run again.
    (
        "cargo publish --workspace --dry-run",
        Some("scripts/publish-crates.sh --dry-run"),
    ),
    // A publish refuses at the detached HEAD a tag is checked out at; the release check
    // requires the tag's commit on main, where CI ran this step on that commit.
    (
        "cargo run -q -p loft-cli -- publish\ntest -z \"$(git status --porcelain)\"",
        None,
    ),
];

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
        .iter()
        .filter(|line| line.starts_with("- { platform: "))
        .map(|line| (field(line, "platform"), field(line, "target")))
        .collect()
}

#[test]
fn a_version_tag_runs_the_release_check_on_the_tag_s_commit_first() {
    let workflow = release();
    assert_eq!(block(&workflow, "on:", " "), ["push:", "tags: [\"v*\"]"]);
    let check = job(&workflow, "check");
    // The whole history, so that the check can find the tag's commit on `origin/main`.
    let checkout = check.position("- uses: actions/checkout@v7");
    assert_eq!(
        check[checkout + 1..checkout + 3],
        ["with:", "fetch-depth: 0"]
    );
    assert!(
        !check.iter().any(|line| line.starts_with("ref:")),
        "{check:?}"
    );
    let release_check = check.position("- run: scripts/release-check.sh \"$GITHUB_REF_NAME\"");
    let first_run = check
        .iter()
        .position(|line| line.starts_with("- run: cargo"));
    assert!(
        first_run.is_some_and(|first| release_check < first),
        "{check:?}"
    );
}

#[test]
fn the_check_job_runs_the_gate_and_the_build_waits_for_it() {
    let workflow = release();
    let gate = job(&read(".github/workflows/ci.yml"), "rust").runs();
    assert!(gate.len() >= 8, "the scan found {gate:?}");
    for (step, _) in RELEASE_FORM {
        assert!(
            gate.iter().any(|run| run == step),
            "CI no longer runs `{step}`"
        );
    }
    let check = job(&workflow, "check").runs();
    for step in &gate {
        let form = RELEASE_FORM.iter().find(|(run, _)| run == step);
        if let Some(expected) = form.map_or(Some(step.as_str()), |(_, form)| *form) {
            assert!(
                check.iter().any(|run| run == expected),
                "the check job lacks `{expected}`"
            );
        }
    }
    for (name, needs) in [
        ("build", "check"),
        ("package", "[build, check]"),
        ("publish", "[package, check]"),
    ] {
        let lines = job(&workflow, name);
        assert_eq!(lines[0], format!("needs: {needs}"), "the job `{name}`");
    }
    assert_eq!(
        job(&workflow, "build")[1..],
        ["uses: ./.github/workflows/build.yml"]
    );
    assert_eq!(
        block(&build(), "on:", " "),
        ["workflow_call:", "workflow_dispatch:"]
    );
}

/// The check job's output `channel`, the npm dist-tag the release check names: `latest` for a
/// release, or a prerelease's identifier.
const CHANNEL: &str = "steps.release.outputs.channel";

#[test]
fn loft_verify_runs_on_the_tag_s_commit_for_a_release_and_not_for_a_prerelease() {
    let workflow = release();
    // CI runs verify, on main only.
    let gate = job(&read(".github/workflows/ci.yml"), "rust").runs();
    assert!(
        gate.iter()
            .any(|run| run == "cargo run -q -p loft-cli -- verify")
    );
    let check = job(&workflow, "check");
    let outputs = check.position("outputs:");
    assert_eq!(check[outputs + 1], format!("channel: ${{{{ {CHANNEL} }}}}"));
    let release_check = check.position("- run: scripts/release-check.sh \"$GITHUB_REF_NAME\"");
    assert_eq!(check[release_check + 1], "id: release");
    let verify = check.position("- run: cargo run -q -p loft-cli -- verify");
    assert_eq!(check[verify + 1], format!("if: {CHANNEL} == 'latest'"));
}

/// The job `name`, asserting it reads the check job's channel as `CHANNEL`.
fn on_the_channel<'a>(workflow: &'a str, name: &str) -> Job<'a> {
    let lines = job(workflow, name);
    lines.position("CHANNEL: ${{ needs.check.outputs.channel }}");
    lines
}

#[test]
fn each_npm_package_is_published_under_the_channel_the_release_check_names() {
    let workflow = release();
    let package = on_the_channel(&workflow, "package");
    package.position("- run: scripts/publish-npm.sh release/npm --tag \"$CHANNEL\" --dry-run");
    let publish = on_the_channel(&workflow, "publish");
    publish.position("- run: scripts/publish-npm.sh release/npm --tag \"$CHANNEL\"");
    assert!(
        !publish
            .iter()
            .chain(package.iter())
            .any(|line| *line == "- run: scripts/publish-npm.sh release/npm"),
        "npm publishes under its default dist-tag"
    );
}

#[test]
fn the_github_release_of_a_prerelease_is_marked_one_and_not_the_latest() {
    let workflow = release();
    let publish = on_the_channel(&workflow, "publish");
    let script = publish
        .runs()
        .into_iter()
        .find(|run| run.contains("gh release create"))
        .expect("the publishing job creates the GitHub release");
    let lines: Vec<&str> = script.lines().collect();
    let marked = lines
        .iter()
        .position(|line| {
            *line == "[ \"$CHANNEL\" = latest ] || kind=\"--prerelease --latest=false\""
        })
        .unwrap_or_else(|| panic!("{script}"));
    let created = lines
        .iter()
        .position(|line| line.starts_with("gh release create "))
        .unwrap();
    assert!(marked < created, "{script}");
    assert_eq!(
        lines[created + 1],
        "--notes-file notes.md $kind",
        "{script}"
    );
}

#[test]
fn the_github_release_carries_an_archive_of_each_platform_s_binary() {
    let workflow = release();
    let builder = build();
    let platforms: Vec<String> = matrix(&builder).into_iter().map(|(p, _)| p).collect();
    assert_eq!(platforms.len(), 6, "{platforms:?}");
    let steps = job(&builder, "build");
    for line in [
        "mkdir -p \"binaries/${{ matrix.platform }}\"",
        "cp \"target/${{ matrix.target }}/release/$executable\" \"binaries/${{ matrix.platform }}/\"",
        "name: binary-${{ matrix.platform }}",
        "path: binaries",
    ] {
        steps.position(line);
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
        package.position(line);
    }
    let publish = job(&workflow, "publish");
    publish.position("gh release upload \"$GITHUB_REF_NAME\" release/archives/* --clobber");
}

#[test]
fn the_build_the_npm_packages_and_their_tests_name_the_same_six_platforms() {
    let supported = supported();
    let mut built = Vec::new();
    for (platform, target) in matrix(&build()) {
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
    let mut tested = PLATFORMS.to_vec();
    tested.sort_unstable();
    assert_eq!(tested, supported);
}

/// The job `package`, asserting the approval comes after it: neither it nor the jobs before it
/// has an environment, and the publishing job, which needs it, runs in the one that requires
/// approval.
fn before_the_approval(workflow: &str) -> Job<'_> {
    for name in ["check", "build", "package"] {
        assert!(!job(workflow, name).has_environment(), "{name}");
    }
    let publish = job(workflow, "publish");
    publish.position("needs: [package, check]");
    publish.position("environment: release");
    job(workflow, "package")
}

#[test]
fn each_npm_package_is_packed_into_a_tarball_before_the_approval() {
    let workflow = release();
    let package = before_the_approval(&workflow);
    let packed = package.position("- run: scripts/npm-packages.sh binaries release/npm");
    let uploaded = package.position("path: release");
    assert!(packed < uploaded, "{package:?}");
}

#[test]
fn loft_runs_from_the_tarballs_before_the_approval() {
    let workflow = release();
    let package = before_the_approval(&workflow);
    let packed = package.position("- run: scripts/npm-packages.sh binaries release/npm");
    let smoke = package.position("- run: scripts/npm-smoke.sh release/npm");
    assert!(packed < smoke);
}

#[test]
fn the_publishing_job_runs_the_tarballs_it_downloaded_before_it_publishes() {
    let workflow = release();
    let publish = job(&workflow, "publish");
    let download = publish.position("- uses: actions/download-artifact@v8");
    assert_eq!(
        publish[download + 1..download + 4],
        ["with:", "name: release", "path: release"]
    );
    let smoke = publish.position("- run: scripts/npm-smoke.sh release/npm");
    let crates = publish.position("- run: scripts/publish-crates.sh");
    let npm = publish.position("- run: scripts/publish-npm.sh release/npm --tag \"$CHANNEL\"");
    let github = publish.position("- name: GitHub release");
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
        let text = read(format!(".github/workflows/{workflow}"));
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
