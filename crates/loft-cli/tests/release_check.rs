//! The release check, `scripts/release-check.sh`, run with real git, Cargo and jq over a scratch
//! repository that stands for this one: a workspace of two published crates and one that is not,
//! a changelog, and a bare `origin` whose `main` and `develop` the scratch repository has pushed.
//! The check passes only when the tag is a release `vX.Y.Z` or a prerelease `vX.Y.Z-alpha.N`,
//! `-beta.N` or `-rc.N`, names the version every published crate carries, its commit is on
//! `main` (or, for a prerelease, on `develop`), and `CHANGELOG.md` dates that version.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use software_plant::real_git::{git, isolate};

use crate::common::{Scratch, stderr, stdout, workspace};

const SCRIPT: &str = "scripts/release-check.sh";

const DATED: &str = "# Changelog\n\n## Unreleased\n\n## 1.2.3 (2026-10-14)\n\n- First.\n";

fn put(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// Writes the manifest of the crate `name` at `version`, published unless `publish` says not.
fn manifest(repo: &Path, name: &str, version: &str, publish: &str) {
    let manifest = format!(
        "[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2021\"\n{publish}"
    );
    put(repo, &format!("{name}/Cargo.toml"), &manifest);
}

/// A scratch repository under `<scratch>/repo`: the crates `one` and `two`, published at
/// 1.2.3, the crate `tool` at 0.0.1 and not published, a changelog that dates 1.2.3, and the
/// release check under `scripts/`, committed on `main` and pushed to the bare `<scratch>/origin`
/// as its `main` and its `develop`.
fn repository() -> (Scratch, PathBuf) {
    let scratch = Scratch::empty();
    let repo = scratch.0.join("repo");
    put(
        &repo,
        "Cargo.toml",
        "[workspace]\nmembers = [\"one\", \"two\", \"tool\"]\nresolver = \"2\"\n",
    );
    for (name, version, publish) in [
        ("one", "1.2.3", ""),
        ("two", "1.2.3", ""),
        ("tool", "0.0.1", "publish = false\n"),
    ] {
        manifest(&repo, name, version, publish);
        put(&repo, &format!("{name}/src/lib.rs"), "");
    }
    put(&repo, "CHANGELOG.md", DATED);
    put(&repo, SCRIPT, "");
    fs::copy(workspace().join(SCRIPT), repo.join(SCRIPT)).unwrap();
    git(&repo, &["init", "--quiet", "-b", "main"]);
    commit(&repo, "release");
    let origin = scratch.0.join("origin");
    git(&scratch.0, &["init", "--quiet", "--bare", "origin"]);
    git(
        &repo,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    git(
        &repo,
        &["push", "--quiet", "origin", "main", "main:develop"],
    );
    (scratch, repo)
}

fn commit(repo: &Path, message: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "--quiet", "--allow-empty", "-m", message]);
}

/// Folds the changes in `repo` into its commit on `main`, pushed in place of the one before, and
/// tags it `tag`, whichever commit held the tag before.
fn amend_and_tag(repo: &Path, tag: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "--quiet", "--amend", "--no-edit"]);
    git(repo, &["push", "--quiet", "--force", "origin", "main"]);
    git(repo, &["tag", "--force", tag]);
}

/// Sets every published crate of `repo` at `version`, dated in its changelog, and commits that
/// on a new branch `branch` from `main`, pushed to `origin` when `push`, tagged `v<version>` and
/// left checked out, as the release workflow checks out the tag.
fn version_on(repo: &Path, version: &str, branch: &str, push: bool) {
    git(repo, &["switch", "--quiet", "-c", branch, "main"]);
    for name in ["one", "two"] {
        manifest(repo, name, version, "");
    }
    put(
        repo,
        "CHANGELOG.md",
        &format!("# Changelog\n\n## {version} (2026-10-28)\n\n{DATED}"),
    );
    commit(repo, version);
    if push {
        git(repo, &["push", "--quiet", "--force", "origin", branch]);
    }
    git(repo, &["tag", &format!("v{version}")]);
}

/// The release check of `tag`, run from the copy in `repo`, which works from its own directory.
fn release_check(repo: &Path, tag: &str) -> Output {
    release_check_reporting(repo, tag, None)
}

/// The release check of `tag`, writing its outputs to `outputs` as GitHub's runner names it in
/// `GITHUB_OUTPUT`, or with no such file.
fn release_check_reporting(repo: &Path, tag: &str, outputs: Option<&Path>) -> Output {
    let mut command = Command::new(repo.join(SCRIPT));
    command.env_remove("GITHUB_OUTPUT");
    if let Some(outputs) = outputs {
        command.env("GITHUB_OUTPUT", outputs);
    }
    isolate(&mut command)
        .arg(tag)
        .current_dir(workspace())
        .output()
        .unwrap()
}

/// Asserts the check refused `tag` with exit code 1 and a report that contains `reason`.
fn assert_refused(repo: &Path, tag: &str, reason: &str) {
    let output = release_check(repo, tag);
    assert_eq!(output.status.code(), Some(1), "{tag}: {}", stdout(&output));
    let report = stderr(&output);
    assert!(
        report.starts_with("release-check: ") && report.contains(reason),
        "{tag}: {report}"
    );
}

#[test]
fn a_tag_on_main_naming_the_dated_version_of_every_published_crate_may_be_released() {
    let (_scratch, repo) = repository();
    // Before the tag exists, the check is of `HEAD`.
    let output = release_check(&repo, "v1.2.3");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    git(&repo, &["tag", "v1.2.3"]);
    let output = release_check(&repo, "v1.2.3");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout(&output), "release-check: v1.2.3 may be released\n");
}

#[test]
fn the_check_fails_unless_the_tag_names_the_version_every_published_crate_carries() {
    let (_scratch, repo) = repository();
    git(&repo, &["tag", "v1.2.4"]);
    assert_refused(&repo, "v1.2.4", "does not name the crates' version 1.2.3");
    manifest(&repo, "two", "1.2.4", "");
    amend_and_tag(&repo, "v1.2.3");
    assert_refused(&repo, "v1.2.3", "carry different versions");
}

#[test]
fn the_check_fails_unless_the_tag_s_commit_is_on_main() {
    let (_scratch, repo) = repository();
    git(&repo, &["switch", "--quiet", "-c", "side"]);
    commit(&repo, "beside main");
    git(&repo, &["tag", "v1.2.3"]);
    git(&repo, &["switch", "--quiet", "main"]);
    assert_refused(&repo, "v1.2.3", "is not on main");
    // A commit on the local `main` that `origin` does not hold is not on `main` either.
    commit(&repo, "unpushed");
    git(&repo, &["tag", "--force", "v1.2.3"]);
    assert_refused(&repo, "v1.2.3", "is not on main");
}

#[test]
fn the_check_fails_unless_the_changelog_dates_the_version() {
    let changelogs = [
        "# Changelog\n\n## Unreleased\n\n- First.\n",
        "# Changelog\n\n## 1.2.3\n\n- First.\n",
        "# Changelog\n\n## 1.2.3 (soon)\n\n- First.\n",
        "# Changelog\n\n## 1.2.2 (2026-10-14)\n\n- First.\n",
    ];
    let (_scratch, repo) = repository();
    for changelog in changelogs {
        put(&repo, "CHANGELOG.md", changelog);
        amend_and_tag(&repo, "v1.2.3");
        assert_refused(&repo, "v1.2.3", "CHANGELOG.md has no dated entry");
    }
}

#[test]
fn the_check_refuses_a_release_whose_commit_is_on_develop_alone() {
    let (_scratch, repo) = repository();
    version_on(&repo, "1.2.4", "develop", true);
    assert_refused(&repo, "v1.2.4", "is not on main");
}

#[test]
fn a_prerelease_tag_on_develop_may_be_released() {
    let (_scratch, repo) = repository();
    version_on(&repo, "1.2.4-alpha.1", "develop", true);
    let output = release_check(&repo, "v1.2.4-alpha.1");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "release-check: v1.2.4-alpha.1 may be released\n"
    );
}

#[test]
fn a_prerelease_tag_on_neither_develop_nor_main_is_refused() {
    let (_scratch, repo) = repository();
    version_on(&repo, "1.2.4-beta.2", "side", true);
    assert_refused(&repo, "v1.2.4-beta.2", "is on neither develop nor main");
    // Nor is one whose commit `origin` does not hold at all.
    version_on(&repo, "1.2.4-rc.1", "unpushed", false);
    assert_refused(&repo, "v1.2.4-rc.1", "is on neither develop nor main");
}

#[test]
fn a_tag_of_another_form_is_refused_naming_the_forms_released() {
    let (_scratch, repo) = repository();
    for tag in [
        "1.2.3",
        "v1.2.3-pre",
        "v1.2.3.4",
        "v1.2.3-alpha",
        "v1.2.3-dev.1",
        "v1.2.3-rc.1.1",
        "v1.2",
    ] {
        assert_refused(
            &repo,
            tag,
            "names neither a release vX.Y.Z nor a prerelease vX.Y.Z-alpha.N, vX.Y.Z-beta.N or vX.Y.Z-rc.N",
        );
    }
}

#[test]
fn the_check_reports_the_npm_channel_of_a_release_and_of_each_prerelease() {
    let (scratch, repo) = repository();
    let outputs = scratch.0.join("outputs");
    let channel_of = |tag: &str| {
        fs::write(&outputs, "earlier=kept\n").unwrap();
        let output = release_check_reporting(&repo, tag, Some(&outputs));
        assert_eq!(output.status.code(), Some(0), "{tag}: {}", stderr(&output));
        fs::read_to_string(&outputs).unwrap()
    };
    git(&repo, &["tag", "v1.2.3"]);
    assert_eq!(channel_of("v1.2.3"), "earlier=kept\nchannel=latest\n");
    for (version, channel) in [
        ("1.2.4-alpha.1", "alpha"),
        ("1.2.4-beta.1", "beta"),
        ("1.2.4-rc.1", "rc"),
    ] {
        version_on(&repo, version, "develop", true);
        assert_eq!(
            channel_of(&format!("v{version}")),
            format!("earlier=kept\nchannel={channel}\n"),
            "{version}"
        );
        git(&repo, &["switch", "--quiet", "main"]);
        git(&repo, &["branch", "--quiet", "-D", "develop"]);
    }
    // Without the runner's file, the check writes no outputs.
    let output = release_check(&repo, "v1.2.3");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
}
