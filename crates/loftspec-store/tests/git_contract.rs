//! The git contract: each scenario runs against real git in a scratch repository and against
//! the Software Plant's git simulator, and the two must answer each call Loft makes alike, byte
//! for byte, and give the repository port the same state.

mod common;
#[path = "common/real_git.rs"]
mod real_git;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::Scratch;
use loftspec::publish::RepoState;
use loftspec_store::StoreError;
use loftspec_store::merge_driver::{DRIVER, driver, install};
use loftspec_store::repository::{
    CURRENT_BRANCH, GitCommand, Repository, SHOW_PREFIX, STATUS, launcher_call,
};
use real_git::{git, real_git, reply, simulated, state};
use software_plant::GitScenario;

/// The calls Loft makes of git, exactly as the repository port makes them.
const CALLS: [&[&str]; 3] = [&SHOW_PREFIX, &CURRENT_BRANCH, &STATUS];

/// A repository whose top level is `dir`, holding `spec/tags.yaml`, `README.md` and an ignore
/// file for `*.tmp`, committed on `main`.
fn committed(dir: &Scratch) -> PathBuf {
    dir.put("spec/tags.yaml", "tags\n");
    dir.put("README.md", "readme\n");
    dir.put(".gitignore", "*.tmp\n");
    git(&dir.0, &["init", "--quiet", "-b", "main"]);
    git(&dir.0, &["add", "."]);
    git(&dir.0, &["commit", "--quiet", "-m", "start"]);
    dir.0.clone()
}

/// `committed`, then `change` applied to it.
fn changed(dir: &Scratch, change: impl FnOnce(&Scratch)) -> PathBuf {
    let root = committed(dir);
    change(dir);
    root
}

fn clean() -> GitScenario {
    GitScenario::clean_on("main")
}

fn outside(dir: &Scratch) -> PathBuf {
    dir.0.clone()
}

fn below_the_top_level(dir: &Scratch) -> PathBuf {
    committed(dir);
    dir.put("sub/spec/a.yaml", "a\n");
    git(&dir.0, &["add", "sub"]);
    git(&dir.0, &["commit", "--quiet", "-m", "sub"]);
    dir.put("sub/spec/a.yaml", "changed\n");
    dir.put("spec/tags.yaml", "changed\n");
    dir.0.join("sub")
}

/// One scenario: how to build it with real git, giving the directory that holds `spec/`, and the
/// same scenario for the simulator.
type Case = (&'static str, fn(&Scratch) -> PathBuf, fn() -> GitScenario);

const CASES: [Case; 14] = [
    ("clean on main", committed, clean),
    (
        "modified",
        |dir| changed(dir, |dir| dir.put("spec/tags.yaml", "changed\n")),
        || clean().with(" M", "spec/tags.yaml"),
    ),
    (
        "staged",
        |dir| {
            changed(dir, |dir| {
                dir.put("spec/tags.yaml", "changed\n");
                git(&dir.0, &["add", "spec/tags.yaml"]);
            })
        },
        || clean().with("M ", "spec/tags.yaml"),
    ),
    (
        "deleted",
        |dir| {
            changed(dir, |dir| {
                fs::remove_file(dir.0.join("spec/tags.yaml")).unwrap();
            })
        },
        || clean().with(" D", "spec/tags.yaml"),
    ),
    (
        "renamed",
        |dir| {
            changed(dir, |dir| {
                git(&dir.0, &["mv", "spec/tags.yaml", "spec/labels.yaml"]);
            })
        },
        || clean().renamed("spec/tags.yaml", "spec/labels.yaml"),
    ),
    (
        "untracked",
        |dir| {
            changed(dir, |dir| {
                dir.put("spec/products/a/new.yaml", "new\n");
                dir.put("spec/b.yaml", "b\n");
            })
        },
        || {
            clean()
                .with("??", "spec/products/a/new.yaml")
                .with("??", "spec/b.yaml")
        },
    ),
    (
        "tracked and untracked",
        |dir| {
            changed(dir, |dir| {
                dir.put("spec/a.yaml", "a\n");
                dir.put("spec/tags.yaml", "changed\n");
            })
        },
        || {
            clean()
                .with("??", "spec/a.yaml")
                .with(" M", "spec/tags.yaml")
        },
    ),
    (
        "ignored",
        |dir| changed(dir, |dir| dir.put("spec/scratch.tmp", "x\n")),
        clean,
    ),
    (
        "hidden",
        |dir| changed(dir, |dir| dir.put("spec/.DS_Store", "x\n")),
        || clean().with("??", "spec/.DS_Store"),
    ),
    (
        "changed outside spec",
        |dir| changed(dir, |dir| dir.put("README.md", "changed\n")),
        || clean().with(" M", "README.md"),
    ),
    (
        "another branch",
        |dir| {
            changed(dir, |dir| {
                git(&dir.0, &["checkout", "--quiet", "-b", "develop"]);
            })
        },
        || GitScenario::clean_on("develop"),
    ),
    (
        "detached",
        |dir| changed(dir, |dir| git(&dir.0, &["checkout", "--quiet", "--detach"])),
        || clean().detached(),
    ),
    ("below the top level", below_the_top_level, || {
        clean()
            .below("sub/")
            .with(" M", "sub/spec/a.yaml")
            .with(" M", "spec/tags.yaml")
    }),
    (
        "outside a repository",
        outside,
        GitScenario::outside_repository,
    ),
];

#[test]
fn ac_965_ac_966_and_ac_967_the_simulator_answers_each_call_as_real_git_does() {
    for (name, build, scenario) in CASES {
        let repository = Scratch::new();
        let root = build(&repository);
        let plant = Scratch::new();
        let simulator = scenario().install(&plant.0);
        for call in CALLS {
            let real = real_git().arg("-C").arg(&root).args(call).output().unwrap();
            let simulated = simulator
                .apply(&mut Command::new("git"))
                .arg("-C")
                .arg(&root)
                .args(call)
                .output()
                .unwrap();
            assert_eq!(reply(&simulated), reply(&real), "{name}: git {call:?}");
        }
    }
}

#[test]
fn ac_960_and_ac_961_the_port_reads_the_same_state_from_the_simulator_and_real_git() {
    for (name, build, scenario) in CASES {
        let repository = Scratch::new();
        let root = build(&repository);
        let plant = Scratch::new();
        let real = GitCommand::from_command(real_git);
        assert_eq!(
            state(&simulated(&scenario(), &plant.0), &root),
            state(&real, &root),
            "{name}"
        );
    }
}

fn on(branch: Option<&str>, modified: &[&str]) -> RepoState {
    RepoState {
        branch: branch.map(str::to_owned),
        modified: modified.iter().map(|path| (*path).to_owned()).collect(),
        stopped: None,
    }
}

#[test]
fn the_port_reads_each_scenario_as_the_publisher_needs_it() {
    let expected = [
        ("clean on main", Ok(on(Some("main"), &[]))),
        ("hidden", Ok(on(Some("main"), &[]))),
        ("ignored", Ok(on(Some("main"), &[]))),
        ("changed outside spec", Ok(on(Some("main"), &[]))),
        (
            "renamed",
            Ok(on(Some("main"), &["spec/labels.yaml", "spec/tags.yaml"])),
        ),
        (
            "below the top level",
            Ok(on(Some("main"), &["spec/a.yaml"])),
        ),
        ("detached", Ok(on(None, &[]))),
        ("outside a repository", Err("not a repository".to_owned())),
    ];
    for (name, want) in expected {
        let (_, build, scenario) = CASES.into_iter().find(|case| case.0 == name).unwrap();
        let plant = Scratch::new();
        assert_eq!(
            state(&simulated(&scenario(), &plant.0), &plant.0),
            want,
            "{name}"
        );
        let repository = Scratch::new();
        let root = build(&repository);
        assert_eq!(
            state(&GitCommand::from_command(real_git), &root),
            want,
            "{name}"
        );
    }
}

#[test]
fn a_git_that_cannot_be_run_is_reported_as_unavailable() {
    let plant = Scratch::new();
    let missing = plant.0.join("no-such-git");
    let git = GitCommand::from_command(move || Command::new(&missing));
    let error = git.state(&plant.0).unwrap_err();
    assert!(matches!(error, StoreError::GitUnavailable(_)), "{error}");
}

#[test]
fn ac_973_a_scripted_git_error_is_reported_with_its_call_and_message() {
    let plant = Scratch::new();
    let scenario = clean().failing(
        software_plant::git::Call::Status,
        128,
        "fatal: index file corrupt\n",
    );
    let error = simulated(&scenario, &plant.0).state(&plant.0).unwrap_err();
    assert!(
        matches!(&error, StoreError::Git { call, message }
            if call.starts_with("status") && message == "fatal: index file corrupt"),
        "{error}"
    );
}

#[test]
fn ac_978_reading_the_state_changes_no_file_of_the_repository() {
    for (name, build, _) in CASES {
        let repository = Scratch::new();
        let root = build(&repository);
        let before = repository.snapshot();
        let _ = GitCommand::from_command(real_git).state(&root);
        let after = repository.snapshot();
        let changed: Vec<&String> = before
            .keys()
            .chain(after.keys())
            .filter(|path| before.get(*path) != after.get(*path))
            .collect();
        assert!(changed.is_empty(), "{name}: changed {changed:?}");
    }
}

/// A launcher that runs anywhere, and one that runs nowhere.
const LAUNCHER: &str = "echo loft 9.9.9 #";
const MISSING: &str = "no-such-loft";

#[test]
fn the_simulator_answers_the_config_calls_as_real_git_does() {
    let (name, value) = (DRIVER, driver(LAUNCHER));
    let value = value.as_str();
    // Unset, then set, then read back: each call sees the configuration the one before left.
    let calls: [&[&str]; 3] = [
        &["config", "--local", "--get", name],
        &["config", "--local", name, value],
        &["config", "--local", "--get", name],
    ];
    let cases: [Case; 2] = [
        ("clean on main", committed, clean),
        (
            "outside a repository",
            outside,
            GitScenario::outside_repository,
        ),
    ];
    for (case, build, scenario) in cases {
        let repository = Scratch::new();
        let root = build(&repository);
        let plant = Scratch::new();
        let simulator = scenario().install(&plant.0);
        for call in calls {
            let real = real_git().arg("-C").arg(&root).args(call).output().unwrap();
            let simulated = simulator
                .apply(&mut Command::new("git"))
                .arg("-C")
                .arg(&root)
                .args(call)
                .output()
                .unwrap();
            assert_eq!(reply(&simulated), reply(&real), "{case}: git {call:?}");
        }
    }
}

#[test]
fn the_simulator_answers_the_launcher_alias_as_real_git_does() {
    let cases: [Case; 2] = [
        ("clean on main", committed, || {
            clean().launcher(LAUNCHER, "loft 9.9.9\n")
        }),
        ("outside a repository", outside, || {
            GitScenario::outside_repository().launcher(LAUNCHER, "loft 9.9.9\n")
        }),
    ];
    for (case, build, scenario) in cases {
        let repository = Scratch::new();
        let root = build(&repository);
        let plant = Scratch::new();
        let simulator = scenario().install(&plant.0);
        for launcher in [LAUNCHER, MISSING] {
            let call = launcher_call(launcher);
            let real = real_git()
                .arg("-C")
                .arg(&root)
                .args(&call)
                .output()
                .unwrap();
            let simulated = simulator
                .apply(&mut Command::new("git"))
                .arg("-C")
                .arg(&root)
                .args(&call)
                .output()
                .unwrap();
            assert_eq!(reply(&simulated), reply(&real), "{case}: {launcher}");
        }
    }
}

/// What one install wrote, or why it was refused.
type Installed = Result<Vec<String>, String>;

/// What two installs with `launcher` write, against the git that `git` runs.
fn installs(git: &GitCommand, root: &Path, launcher: &str) -> [Installed; 2] {
    let once = || {
        let installed = install(git, root, launcher).map_err(|error| match error {
            StoreError::NotARepository(_) => "not a repository".to_owned(),
            other => other.to_string(),
        })?;
        assert_eq!(installed.version, "loft 9.9.9");
        Ok(installed.written)
    };
    [once(), once()]
}

#[test]
fn installing_the_driver_writes_the_same_against_real_git_and_the_simulator() {
    let all = || {
        Ok(vec![
            ".gitattributes".to_owned(),
            "merge.loft.driver".to_owned(),
            "merge.loft.name".to_owned(),
        ])
    };
    let refused = || Err("not a repository".to_owned());
    let no_launcher = || {
        Err(format!(
            "git could not run `{MISSING} --version` from the top of the working tree"
        ))
    };
    let with_launcher = || clean().launcher(LAUNCHER, "loft 9.9.9\n");
    let cases: [(Case, &str, [Installed; 2]); 3] = [
        (
            ("clean on main", committed, with_launcher),
            LAUNCHER,
            [all(), Ok(Vec::new())],
        ),
        (
            ("no launcher", committed, clean),
            MISSING,
            [no_launcher(), no_launcher()],
        ),
        (
            (
                "outside a repository",
                outside,
                GitScenario::outside_repository,
            ),
            LAUNCHER,
            [refused(), refused()],
        ),
    ];
    for ((name, build, scenario), launcher, want) in cases {
        let repository = Scratch::new();
        let root = build(&repository);
        let before = repository.snapshot();
        assert_eq!(
            installs(&GitCommand::from_command(real_git), &root, launcher),
            want,
            "{name}: git"
        );
        if want[0].is_err() {
            assert!(
                repository.snapshot() == before,
                "{name}: nothing is written"
            );
        }
        let plant = Scratch::new();
        let simulated = simulated(&scenario(), &plant.0);
        let tree = Scratch::new();
        assert_eq!(
            installs(&simulated, &tree.0, launcher),
            want,
            "{name}: simulator"
        );
    }
}
