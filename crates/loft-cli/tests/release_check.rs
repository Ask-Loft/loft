//! The release check, `scripts/release-check.sh`, run with real git, Cargo and jq over a scratch
//! repository that stands for this one: a workspace of two published crates and one that is not,
//! a changelog, and a bare `origin` whose `main` the scratch repository has pushed. The check
//! passes only when the tag names the version every published crate carries, the tag's commit is
//! on `main`, and `CHANGELOG.md` dates that version.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use software_plant::real_git::{isolate, setup_git};

use crate::common::{Scratch, stderr, stdout, workspace};

const SCRIPT: &str = "scripts/release-check.sh";

const DATED: &str = "# Changelog\n\n## Unreleased\n\n## 1.2.3 (2026-10-14)\n\n- First.\n";

fn put(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn git(repo: &Path, args: &[&str]) {
    let output = setup_git().arg("-C").arg(repo).args(args).output().unwrap();
    assert!(output.status.success(), "git {args:?}: {}", stderr(&output));
}

/// A scratch repository under `<scratch>/repo`: the crates `one` and `two`, published at
/// `versions`, the crate `tool` at 0.0.1 and not published, `changelog` as `CHANGELOG.md`, and the
/// release check under `scripts/`, committed on `main` and pushed to the bare `<scratch>/origin`.
fn repository(versions: [&str; 2], changelog: &str) -> (Scratch, PathBuf) {
    let scratch = Scratch::empty();
    let repo = scratch.0.join("repo");
    put(
        &repo,
        "Cargo.toml",
        "[workspace]\nmembers = [\"one\", \"two\", \"tool\"]\nresolver = \"2\"\n",
    );
    let crates = [
        ("one", versions[0], ""),
        ("two", versions[1], ""),
        ("tool", "0.0.1", "publish = false\n"),
    ];
    for (name, version, publish) in crates {
        let manifest = format!(
            "[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2021\"\n{publish}"
        );
        put(&repo, &format!("{name}/Cargo.toml"), &manifest);
        put(&repo, &format!("{name}/src/lib.rs"), "");
    }
    put(&repo, "CHANGELOG.md", changelog);
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
    git(&repo, &["push", "--quiet", "origin", "main"]);
    (scratch, repo)
}

fn commit(repo: &Path, message: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "--quiet", "--allow-empty", "-m", message]);
}

/// The release check of `tag`, run from the copy in `repo`, which works from its own directory.
fn release_check(repo: &Path, tag: &str) -> Output {
    let mut command = Command::new(repo.join(SCRIPT));
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
    let (_scratch, repo) = repository(["1.2.3", "1.2.3"], DATED);
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
    let (_scratch, repo) = repository(["1.2.3", "1.2.3"], DATED);
    git(&repo, &["tag", "v1.2.4"]);
    assert_refused(&repo, "v1.2.4", "does not name the crates' version 1.2.3");
    assert_refused(&repo, "1.2.3", "does not name the crates' version 1.2.3");
    let (_scratch, repo) = repository(["1.2.3", "1.2.4"], DATED);
    git(&repo, &["tag", "v1.2.3"]);
    assert_refused(&repo, "v1.2.3", "carry different versions");
}

#[test]
fn the_check_fails_unless_the_tag_s_commit_is_on_main() {
    let (_scratch, repo) = repository(["1.2.3", "1.2.3"], DATED);
    git(&repo, &["switch", "--quiet", "-c", "side"]);
    commit(&repo, "beside main");
    git(&repo, &["tag", "v1.2.3"]);
    git(&repo, &["switch", "--quiet", "main"]);
    assert_refused(&repo, "v1.2.3", "is not on main");
    // A commit on the local `main` that `origin` does not hold is not on `main` either.
    let (_scratch, repo) = repository(["1.2.3", "1.2.3"], DATED);
    commit(&repo, "unpushed");
    git(&repo, &["tag", "v1.2.3"]);
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
    for changelog in changelogs {
        let (_scratch, repo) = repository(["1.2.3", "1.2.3"], changelog);
        git(&repo, &["tag", "v1.2.3"]);
        assert_refused(&repo, "v1.2.3", "CHANGELOG.md has no dated entry");
    }
}
