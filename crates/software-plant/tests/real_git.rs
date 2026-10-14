//! Real git as the contract suites run it reaches only the repository around its working
//! directory: never one that git's environment names, as git names the repository it runs a
//! hook or `git rebase --exec` in.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use software_plant::real_git::{forget_repository, real_git, setup_git};

/// A scratch directory holding two repositories, `here` and `elsewhere`.
struct Two(PathBuf);

impl Two {
    fn new(name: &str) -> Self {
        let base = std::env::temp_dir().join(format!("software-plant-{name}-{}", process::id()));
        let _ = fs::remove_dir_all(&base);
        for repository in ["here", "elsewhere"] {
            let dir = base.join(repository);
            fs::create_dir_all(&dir).unwrap();
            let output = setup_git()
                .arg("-C")
                .arg(&dir)
                .args(["init", "--quiet", "-b", "main"])
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        }
        Self(fs::canonicalize(base).unwrap())
    }

    fn git_dir(&self, repository: &str) -> PathBuf {
        self.0.join(repository).join(".git")
    }
}

impl Drop for Two {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The repository `command`, run in `dir`, finds.
fn found(command: &mut Command, dir: &Path) -> PathBuf {
    let output = command
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim_end())
}

#[test]
fn a_command_that_forgets_the_repository_finds_the_one_around_it_whatever_git_dir_names() {
    let two = Two::new("forget");
    let mut command = Command::new("git");
    command
        .env("GIT_DIR", two.git_dir("elsewhere"))
        .env("GIT_WORK_TREE", two.0.join("elsewhere"))
        .env("GIT_INDEX_FILE", two.git_dir("elsewhere").join("index"));
    forget_repository(&mut command);
    assert_eq!(
        found(&mut command, &two.0.join("here")),
        two.git_dir("here")
    );
}

#[test]
fn real_git_removes_every_variable_that_names_a_repository() {
    let removed: Vec<String> = real_git()
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect();
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
    ] {
        assert!(
            removed.iter().any(|removed| removed == name),
            "{name}: {removed:?}"
        );
    }
}
