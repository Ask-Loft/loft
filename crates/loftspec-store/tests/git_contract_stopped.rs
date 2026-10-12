//! The git contract for a merge or rebase stopped on a conflict: each scenario stops real git in
//! a scratch repository and scripts the simulator with the commits and blobs real git made, and
//! the two must answer each call Loft makes alike and give the repository port the same state.

mod common;
#[path = "common/real_git.rs"]
mod real_git;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::Scratch;
use loftspec::publish::{Operation, RepoState, Stopped};
use loftspec_store::repository::{
    GIT_PATH, GitCommand, MERGE_HEAD, REBASE_HEAD, REBASE_HEAD_NAMES, Repository as _, UNMERGED,
};
use real_git::{git, git_output, real_git, reply, simulated, state};
use software_plant::GitScenario;

/// Runs a setup command of real git in `dir` that stops on a conflict.
fn stops(dir: &Path, args: &[&str]) {
    let output = git_output(dir, args);
    assert!(!output.status.success(), "git {args:?} stops");
}

/// What `git rev-parse` prints for `revision`, without its newline.
fn rev(dir: &Path, revision: &str) -> String {
    let output = real_git()
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", revision])
        .output()
        .unwrap();
    String::from_utf8(output.stdout)
        .unwrap()
        .trim_end()
        .to_owned()
}

/// Which files each side changes, and where `spec/` sits from the top level.
struct Sides {
    sub: &'static str,
    /// The lock at the start; none when the start holds no lock.
    lock: bool,
    /// The local side changes the lock; the pulled side always does.
    local_lock: bool,
    readme: bool,
}

const TOP: Sides = Sides {
    sub: "",
    lock: true,
    local_lock: true,
    readme: false,
};

/// A repository on `main` whose last commit and the branch `pulled` both changed the bindings,
/// and the lock and the README as `sides` says.
fn two_sides(dir: &Scratch, sides: &Sides) {
    let sub = sides.sub;
    dir.put(&format!("{sub}spec/labels.yaml"), "start\n");
    dir.put("README.md", "start\n");
    if sides.lock {
        dir.put(&format!("{sub}loft.lock.json"), "start\n");
    }
    git(&dir.0, &["init", "--quiet", "-b", "main"]);
    git(&dir.0, &["add", "."]);
    git(&dir.0, &["commit", "--quiet", "-m", "start"]);
    for (branch, side) in [("pulled", "pulled"), ("main", "local")] {
        let switch: &[&str] = if branch == "pulled" {
            &["checkout", "--quiet", "-b", "pulled"]
        } else {
            &["checkout", "--quiet", "main"]
        };
        git(&dir.0, switch);
        dir.put(&format!("{sub}spec/labels.yaml"), &format!("{side}\n"));
        if side == "pulled" || sides.local_lock {
            dir.put(&format!("{sub}loft.lock.json"), &format!("{side}\n"));
        }
        if sides.readme {
            dir.put("README.md", &format!("{side}\n"));
        }
        git(&dir.0, &["add", "."]);
        git(&dir.0, &["commit", "--quiet", "-m", side]);
    }
}

/// The scenario's index entries of `paths`, as real git staged them: the merge base, `HEAD` and
/// `other`.
fn unmerged(scenario: GitScenario, dir: &Path, other: &str, paths: &[&str]) -> GitScenario {
    let base = rev(dir, &format!("{other}^"));
    let mut scenario = scenario;
    for path in paths {
        for (stage, revision) in [(1, base.as_str()), (2, "HEAD"), (3, other)] {
            let object = rev(dir, &format!("{revision}:{path}"));
            scenario = scenario.unmerged("100644", &object, stage, path);
        }
    }
    scenario
}

fn merge(dir: &Scratch) -> (PathBuf, GitScenario) {
    two_sides(dir, &TOP);
    stops(&dir.0, &["merge", "--quiet", "pulled"]);
    let scenario = GitScenario::clean_on("main")
        .with("UU", "spec/labels.yaml")
        .merging(&rev(&dir.0, "MERGE_HEAD"))
        .lock_at("HEAD", &rev(&dir.0, "HEAD:loft.lock.json"), "local\n")
        .lock_at(
            "MERGE_HEAD",
            &rev(&dir.0, "MERGE_HEAD:loft.lock.json"),
            "pulled\n",
        );
    let paths = ["loft.lock.json", "spec/labels.yaml"];
    (
        dir.0.clone(),
        unmerged(scenario, &dir.0, "MERGE_HEAD", &paths),
    )
}

fn merge_below_the_top_level(dir: &Scratch) -> (PathBuf, GitScenario) {
    let sides = Sides {
        sub: "sub/",
        readme: true,
        ..TOP
    };
    two_sides(dir, &sides);
    stops(&dir.0, &["merge", "--quiet", "pulled"]);
    let scenario = GitScenario::clean_on("main")
        .below("sub/")
        .with("UU", "sub/spec/labels.yaml")
        .merging(&rev(&dir.0, "MERGE_HEAD"))
        .lock_at("HEAD", &rev(&dir.0, "HEAD:sub/loft.lock.json"), "local\n")
        .lock_at(
            "MERGE_HEAD",
            &rev(&dir.0, "MERGE_HEAD:sub/loft.lock.json"),
            "pulled\n",
        );
    let paths = ["README.md", "sub/loft.lock.json", "sub/spec/labels.yaml"];
    (
        dir.0.join("sub"),
        unmerged(scenario, &dir.0, "MERGE_HEAD", &paths),
    )
}

fn merge_without_a_local_lock(dir: &Scratch) -> (PathBuf, GitScenario) {
    let sides = Sides {
        lock: false,
        local_lock: false,
        ..TOP
    };
    two_sides(dir, &sides);
    stops(&dir.0, &["merge", "--quiet", "pulled"]);
    let scenario = GitScenario::clean_on("main")
        .with("UU", "spec/labels.yaml")
        .merging(&rev(&dir.0, "MERGE_HEAD"))
        .lock_at(
            "MERGE_HEAD",
            &rev(&dir.0, "MERGE_HEAD:loft.lock.json"),
            "pulled\n",
        );
    let paths = ["spec/labels.yaml"];
    (
        dir.0.clone(),
        unmerged(scenario, &dir.0, "MERGE_HEAD", &paths),
    )
}

/// `two_sides` with `main` rebased onto `pulled`, from `main` or from a detached `HEAD`.
fn rebase(dir: &Scratch, detached: bool) -> (PathBuf, GitScenario) {
    two_sides(dir, &TOP);
    if detached {
        git(&dir.0, &["checkout", "--quiet", "--detach", "main"]);
    }
    stops(&dir.0, &["rebase", "--quiet", "pulled"]);
    let head_name = if detached {
        "detached HEAD"
    } else {
        "refs/heads/main"
    };
    let scenario = GitScenario::clean_on("main")
        .detached()
        .with("UU", "spec/labels.yaml")
        .rebasing(&rev(&dir.0, "REBASE_HEAD"), head_name)
        .lock_at("HEAD", &rev(&dir.0, "HEAD:loft.lock.json"), "pulled\n")
        .lock_at(
            "REBASE_HEAD",
            &rev(&dir.0, "REBASE_HEAD:loft.lock.json"),
            "local\n",
        );
    let paths = ["loft.lock.json", "spec/labels.yaml"];
    (
        dir.0.clone(),
        unmerged(scenario, &dir.0, "REBASE_HEAD", &paths),
    )
}

fn stopped(operation: Operation, branch: Option<&str>, locks: [Option<&str>; 2]) -> Stopped {
    let [head, other] = locks.map(|lock| lock.map(|text| text.as_bytes().to_vec()));
    Stopped {
        operation,
        branch: branch.map(str::to_owned),
        head_lock: head,
        other_lock: other,
        unmerged: Vec::new(),
    }
}

/// One scenario: how real git stops, giving the directory that holds `spec/` and the scenario
/// that scripts the simulator alike, and the state the port reads of it.
type Case = (
    &'static str,
    fn(&Scratch) -> (PathBuf, GitScenario),
    fn() -> RepoState,
);

const CASES: [Case; 5] = [
    ("merge", merge, || RepoState {
        branch: Some("main".to_owned()),
        modified: vec!["spec/labels.yaml".to_owned()],
        stopped: Some(Stopped {
            unmerged: vec!["loft.lock.json".to_owned(), "spec/labels.yaml".to_owned()],
            ..stopped(
                Operation::Merge,
                Some("main"),
                [Some("local\n"), Some("pulled\n")],
            )
        }),
    }),
    (
        "merge below the top level",
        merge_below_the_top_level,
        || RepoState {
            branch: Some("main".to_owned()),
            modified: vec!["spec/labels.yaml".to_owned()],
            stopped: Some(Stopped {
                unmerged: vec![
                    ":/README.md".to_owned(),
                    "loft.lock.json".to_owned(),
                    "spec/labels.yaml".to_owned(),
                ],
                ..stopped(
                    Operation::Merge,
                    Some("main"),
                    [Some("local\n"), Some("pulled\n")],
                )
            }),
        },
    ),
    (
        "merge without a local lock",
        merge_without_a_local_lock,
        || RepoState {
            branch: Some("main".to_owned()),
            modified: vec!["spec/labels.yaml".to_owned()],
            stopped: Some(Stopped {
                unmerged: vec!["spec/labels.yaml".to_owned()],
                ..stopped(Operation::Merge, Some("main"), [None, Some("pulled\n")])
            }),
        },
    ),
    (
        "rebase",
        |dir| rebase(dir, false),
        || RepoState {
            branch: None,
            modified: vec!["spec/labels.yaml".to_owned()],
            stopped: Some(Stopped {
                unmerged: vec!["loft.lock.json".to_owned(), "spec/labels.yaml".to_owned()],
                ..stopped(
                    Operation::Rebase,
                    Some("main"),
                    [Some("pulled\n"), Some("local\n")],
                )
            }),
        },
    ),
    (
        "detached rebase",
        |dir| rebase(dir, true),
        || RepoState {
            branch: None,
            modified: vec!["spec/labels.yaml".to_owned()],
            stopped: Some(Stopped {
                unmerged: vec!["loft.lock.json".to_owned(), "spec/labels.yaml".to_owned()],
                ..stopped(Operation::Rebase, None, [Some("pulled\n"), Some("local\n")])
            }),
        },
    ),
];

/// The calls with which the port reads a stopped merge or rebase, each git path but a rebase's,
/// whose path names the simulator's own git directory.
fn calls(rebasing: bool) -> Vec<Vec<String>> {
    let owned = |call: &[&str]| call.iter().map(|arg| (*arg).to_owned()).collect();
    let other = if rebasing {
        "REBASE_HEAD"
    } else {
        "MERGE_HEAD"
    };
    let mut calls: Vec<Vec<String>> =
        vec![owned(&MERGE_HEAD), owned(&REBASE_HEAD), owned(&UNMERGED)];
    for revision in ["HEAD", other] {
        let lock = format!("{revision}:./loft.lock.json");
        calls.push(owned(&["rev-parse", "-q", "--verify", &lock]));
    }
    if !rebasing {
        for name in REBASE_HEAD_NAMES {
            calls.push(owned(&[GIT_PATH[0], GIT_PATH[1], name]));
        }
    }
    calls
}

fn run(mut command: Command, root: &Path, call: &[String]) -> std::process::Output {
    command.arg("-C").arg(root).args(call).output().unwrap()
}

#[test]
fn the_simulator_answers_each_call_of_a_stopped_merge_or_rebase_as_real_git_does() {
    for (name, build, _) in CASES {
        let repository = Scratch::new();
        let (root, scenario) = build(&repository);
        let plant = Scratch::new();
        let simulator = scenario.install(&plant.0);
        let rebasing = name.contains("rebase");
        let compare = |call: &[String]| {
            let real = run(real_git(), &root, call);
            let mut command = Command::new("git");
            simulator.apply(&mut command);
            let simulated = run(command, &root, call);
            assert_eq!(reply(&simulated), reply(&real), "{name}: git {call:?}");
            real
        };
        for call in calls(rebasing) {
            let real = compare(&call);
            // A lock's object id leads to the call that reads it.
            let names_a_lock = call
                .last()
                .is_some_and(|arg| arg.ends_with(":./loft.lock.json"));
            if names_a_lock && real.status.success() {
                let object = String::from_utf8(real.stdout).unwrap();
                compare(&["cat-file", "blob", object.trim_end()].map(str::to_owned));
            }
        }
    }
}

/// What the file that `rebase-merge/head-name`'s git path names holds, from `root`.
fn head_name(command: Command, root: &Path) -> String {
    let call = [GIT_PATH[0], GIT_PATH[1], REBASE_HEAD_NAMES[0]].map(str::to_owned);
    let output = run(command, root, &call);
    let path = String::from_utf8(output.stdout).unwrap();
    fs::read_to_string(root.join(path.trim_end())).unwrap()
}

#[test]
fn the_git_path_of_a_stopped_rebase_names_a_file_that_holds_the_same_branch() {
    for (name, build, _) in CASES.into_iter().filter(|case| case.0.contains("rebase")) {
        let repository = Scratch::new();
        let (root, scenario) = build(&repository);
        let plant = Scratch::new();
        let simulator = scenario.install(&plant.0);
        let mut command = Command::new("git");
        simulator.apply(&mut command);
        assert_eq!(
            head_name(command, &root),
            head_name(real_git(), &root),
            "{name}"
        );
    }
}

#[test]
fn the_port_reads_a_stopped_merge_or_rebase_alike_from_the_simulator_and_real_git() {
    for (name, build, want) in CASES {
        let repository = Scratch::new();
        let (root, scenario) = build(&repository);
        let plant = Scratch::new();
        let real = state(&GitCommand::from_command(real_git), &root);
        assert_eq!(real, Ok(want()), "{name}: git");
        assert_eq!(
            state(&simulated(&scenario, &plant.0), &root),
            real,
            "{name}: simulator"
        );
    }
}

#[test]
fn reading_a_stopped_merge_or_rebase_changes_no_file_of_the_repository() {
    for (name, build, _) in CASES {
        let repository = Scratch::new();
        let (root, _) = build(&repository);
        let before = repository.snapshot();
        let _ = GitCommand::from_command(real_git).state(&root);
        assert!(repository.snapshot() == before, "{name}");
    }
}
