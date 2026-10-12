//! The git contract: each scenario runs against real git in a scratch repository and against
//! the Software Plant's git simulator, and the two must answer each call Loft makes alike, byte
//! for byte, and give the repository port the same state.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::Scratch;
use loftspec::publish::RepoState;
use loftspec_store::StoreError;
use loftspec_store::repository::{CURRENT_BRANCH, GitCommand, Repository, SHOW_PREFIX, STATUS};
use software_plant::GitScenario;

/// The calls Loft makes of git, exactly as the repository port makes them.
const CALLS: [&[&str]; 3] = [&SHOW_PREFIX, &CURRENT_BRANCH, &STATUS];

/// Real git, kept from the user's configuration and from any repository above the temporary
/// directory.
fn real_git() -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CEILING_DIRECTORIES",
            std::env::temp_dir().canonicalize().unwrap(),
        );
    command
}

/// Runs a setup command of real git in `dir`.
fn git(dir: &Path, args: &[&str]) {
    let output = real_git()
        .args([
            "-c",
            "user.name=Plant",
            "-c",
            "user.email=plant@example.invalid",
        ])
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

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

/// What one call printed and how it exited; the error message only where git exits 128, the
/// one failure whose message Loft reads.
fn reply(output: &Output) -> (Option<i32>, Vec<u8>, String) {
    let code = output.status.code();
    let stderr = if code == Some(128) {
        String::from_utf8_lossy(&output.stderr).into_owned()
    } else {
        String::new()
    };
    (code, output.stdout.clone(), stderr)
}

/// The port's answer, with an error reduced to what the publisher acts on.
fn state(git: &GitCommand, root: &Path) -> Result<RepoState, String> {
    git.state(root).map_err(|error| match error {
        StoreError::NotARepository(_) => "not a repository".to_owned(),
        other => other.to_string(),
    })
}

fn simulated(scenario: &GitScenario, plant: &Path) -> GitCommand {
    let simulator = scenario.install(plant);
    GitCommand::from_command(move || {
        let mut command = Command::new("git");
        simulator.apply(&mut command);
        command
    })
}

#[test]
fn the_simulator_answers_each_call_as_real_git_does() {
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
fn the_port_reads_the_same_state_from_the_simulator_and_real_git() {
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
fn a_scripted_git_error_is_reported_with_its_call_and_message() {
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
fn reading_the_state_changes_no_file_of_the_repository() {
    for (name, build, _) in CASES {
        let repository = Scratch::new();
        let root = build(&repository);
        let before = repository.snapshot();
        let _ = GitCommand::from_command(real_git).state(&root);
        assert!(repository.snapshot() == before, "{name}: a file changed");
    }
}
