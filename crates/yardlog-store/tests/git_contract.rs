//! The git contract of Yard Client's calls: each scenario runs against real git in a scratch
//! repository and against the Software Plant's git simulator, and the two must answer each call
//! alike, byte for byte, and give the store the same place, person, branch cut from and answers.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::common::{Scratch, simulated};
use software_plant::GitScenario;
use software_plant::real_git::{git, real_git};
use yardlog_store::git::{
    COMMON_DIR, CONFIG_GET, CURRENT_BRANCH, GIT_PATH, LOCATE, REFLOG, SHOW_REF, TOP_LEVEL, VERIFY,
    YARD_PATHSPEC, YardGit,
};
use yardlog_store::{Place, YardStoreError};

/// The blob of a file that holds `root\n`.
const ROOT_BLOB: &str = "d8649da39ddf7910d29982e2f19cd9c0ff5ffe96";

/// A repository whose top level is `dir/repo`, holding `.yard/main/yardlog.yaml` committed on
/// `main`, and the directory the calls run in.
fn committed(dir: &Scratch) -> PathBuf {
    let repo = dir.0.join("repo");
    dir.put("repo/.yard/main/yardlog.yaml", "root\n");
    dir.put("repo/sub/deep/file", "x\n");
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "start"]);
    repo
}

/// The reflog of `main` once `committed` made its one commit.
const STARTED: [&str; 1] = ["commit (initial): start"];

fn clean(_: &Path) -> GitScenario {
    GitScenario::clean_on("main")
        .holding("HEAD", ".yard/main/yardlog.yaml", ROOT_BLOB)
        .with_reflog("main", &STARTED)
}

/// `committed`, then `topic` created from `from` by `git switch -c` and checked out.
fn switched(dir: &Scratch, from: &str) -> PathBuf {
    let repo = committed(dir);
    git(&repo, &["switch", "--quiet", "-c", "topic", from]);
    repo
}

/// The simulator's `topic`, created from `main` as `committed` left it, whose reflog is
/// `subjects`.
fn topic(subjects: &[&str]) -> GitScenario {
    GitScenario::clean_on("topic")
        .holding("HEAD", ".yard/main/yardlog.yaml", ROOT_BLOB)
        .with_branch("main")
        .with_reflog("main", &STARTED)
        .with_reflog("topic", subjects)
}

/// One scenario: how to build it with real git, giving the directory the calls run in, and the
/// same scenario for the simulator, given the scratch directory.
type Case = (
    &'static str,
    fn(&Scratch) -> PathBuf,
    fn(&Path) -> GitScenario,
);

const CASES: [Case; 11] = [
    ("clean on main", committed, clean),
    (
        "below the top level",
        |dir| committed(dir).join("sub/deep"),
        |dir| clean(dir).below("sub/deep/"),
    ),
    (
        // git ignores what it does not track: `.yard/` is taken out of the index first.
        "ignoring .yard",
        |dir| {
            let repo = committed(dir);
            dir.put("repo/.gitignore", ".yard/\n");
            git(&repo, &["rm", "-r", "--quiet", "--cached", ".yard"]);
            git(&repo, &["add", ".gitignore"]);
            git(&repo, &["commit", "--quiet", "-m", "ignore"]);
            repo
        },
        |_| {
            GitScenario::clean_on("main")
                .ignoring(YARD_PATHSPEC)
                .with_reflog("main", &["commit: ignore", STARTED[0]])
        },
    ),
    (
        // A tracked file is not subject to the ignore rules.
        "ignoring a tracked .yard",
        |dir| {
            let repo = committed(dir);
            dir.put("repo/.gitignore", ".yard/\n");
            repo
        },
        clean,
    ),
    (
        "with a person",
        |dir| {
            let repo = committed(dir);
            git(&repo, &["config", "--local", "user.name", "Ada"]);
            git(
                &repo,
                &["config", "--local", "user.email", "ada@example.com"],
            );
            repo
        },
        |dir| {
            clean(dir)
                .with_config("user.name", "Ada")
                .with_config("user.email", "ada@example.com")
        },
    ),
    (
        "detached",
        |dir| {
            let repo = committed(dir);
            git(&repo, &["checkout", "--quiet", "--detach"]);
            repo
        },
        |dir| clean(dir).detached().with_branch("main"),
    ),
    (
        "a linked worktree",
        |dir| {
            let repo = committed(dir);
            git(
                &repo,
                &["worktree", "add", "--quiet", "-b", "other", "../wt"],
            );
            dir.0.join("wt")
        },
        |dir| {
            let common = dir.join("repo/.git");
            GitScenario::clean_on("other")
                .holding("HEAD", ".yard/main/yardlog.yaml", ROOT_BLOB)
                .with_branch("main")
                .with_reflog("main", &STARTED)
                .with_reflog("other", &["branch: Created from HEAD"])
                .common_dir(
                    &common.to_string_lossy(),
                    &common.join("worktrees/wt").to_string_lossy(),
                )
        },
    ),
    (
        "outside a repository",
        |dir| dir.0.clone(),
        |_| GitScenario::outside_repository(),
    ),
    (
        "cut from a local branch",
        |dir| {
            let repo = committed(dir);
            git(&repo, &["branch", "feat"]);
            git(&repo, &["switch", "--quiet", "-c", "topic", "feat"]);
            repo
        },
        |_| {
            topic(&["branch: Created from feat"])
                .with_branch("feat")
                .with_reflog("feat", &["branch: Created from main"])
        },
    ),
    (
        "cut from a revision that names no branch",
        |dir| switched(dir, "HEAD~0"),
        |_| topic(&["branch: Created from HEAD~0"]),
    ),
    (
        "cut from a branch, its reflog emptied",
        |dir| {
            let repo = switched(dir, "main");
            git(
                &repo,
                &["reflog", "delete", "--rewrite", "refs/heads/topic@{0}"],
            );
            repo
        },
        |_| topic(&[]),
    ),
];

/// Each call Yard Client makes of git, exactly as the store makes it.
fn calls() -> Vec<Vec<&'static str>> {
    let with = |call: &[&'static str], operand: &'static str| {
        call.iter().copied().chain([operand]).collect::<Vec<_>>()
    };
    let reflog = |reference: &'static str| {
        REFLOG
            .iter()
            .copied()
            .chain([reference, "--"])
            .collect::<Vec<_>>()
    };
    vec![
        reflog("refs/heads/main"),
        reflog("refs/heads/topic"),
        reflog("refs/heads/other"),
        reflog("refs/heads/nope"),
        with(&SHOW_REF, "refs/heads/main"),
        with(&SHOW_REF, "refs/heads/feat"),
        with(&SHOW_REF, "refs/heads/topic"),
        with(&SHOW_REF, "refs/heads/HEAD"),
        TOP_LEVEL.to_vec(),
        CURRENT_BRANCH.to_vec(),
        COMMON_DIR.to_vec(),
        with(&GIT_PATH, "yard"),
        LOCATE.to_vec(),
        vec!["check-ignore", "-q", YARD_PATHSPEC],
        with(&CONFIG_GET, "user.name"),
        with(&CONFIG_GET, "user.email"),
        with(&VERIFY, "HEAD:.yard/main/yardlog.yaml"),
        with(&VERIFY, "HEAD:.yard/other/yardlog.yaml"),
    ]
}

/// What one call printed and how it exited; the error message only where git exits 128.
fn reply(output: &Output) -> (Option<i32>, Vec<u8>, String) {
    let code = output.status.code();
    let stderr = if code == Some(128) {
        String::from_utf8_lossy(&output.stderr).into_owned()
    } else {
        String::new()
    };
    (code, output.stdout.clone(), stderr)
}

#[test]
fn the_simulator_answers_each_of_yard_client_s_calls_as_real_git_does() {
    for (name, build, scenario) in CASES {
        let repository = Scratch::new();
        let dir = build(&repository);
        let plant = Scratch::new();
        let simulator = scenario(&repository.0).install(&plant.0);
        for call in calls() {
            let real = real_git().arg("-C").arg(&dir).args(&call).output().unwrap();
            let simulated = simulator
                .apply(&mut Command::new("git"))
                .arg("-C")
                .arg(&dir)
                .args(&call)
                .output()
                .unwrap();
            assert_eq!(reply(&simulated), reply(&real), "{name}: git {call:?}");
        }
    }
}

/// What the store learns of a worktree: where it is, whether `.yard/` is ignored, the person,
/// whether HEAD holds each of two Yard Logs, and the local branch its branch was cut from.
type Learned = (Place, bool, Vec<Option<String>>, [bool; 2], Option<String>);

/// What the store learns of a scenario, with an error reduced to what a command acts on.
fn learned(git: &YardGit, dir: &Path) -> Result<Learned, String> {
    let reduce = |error: YardStoreError| match error {
        YardStoreError::NotARepository(_) => "not a repository".to_owned(),
        other => other.to_string(),
    };
    let place = Place::locate(git, dir).map_err(reduce)?;
    let person = git.person(dir).map_err(reduce)?;
    let holds = |path| git.holds(dir, "HEAD", path).map_err(reduce);
    let cut_from = match &place.branch {
        Some(branch) => git.cut_from(dir, branch).map_err(reduce)?,
        None => None,
    };
    Ok((
        place,
        git.ignores_yard(dir).map_err(reduce)?,
        vec![person.name, person.email],
        [
            holds(".yard/main/yardlog.yaml")?,
            holds(".yard/other/yardlog.yaml")?,
        ],
        cut_from,
    ))
}

#[test]
fn the_store_learns_the_same_from_the_simulator_and_real_git() {
    for (name, build, scenario) in CASES {
        let repository = Scratch::new();
        let dir = build(&repository);
        let plant = Scratch::new();
        let real = YardGit::from_command(real_git);
        assert_eq!(
            learned(&simulated(&scenario(&repository.0), &plant.0), &dir),
            learned(&real, &dir),
            "{name}"
        );
    }
}

#[test]
fn a_linked_worktree_is_its_own_tree_with_its_own_git_directory_and_the_repository_s_common_one() {
    let repository = Scratch::new();
    let (_, build, _) = CASES[6];
    let dir = build(&repository);
    let place = Place::locate(&YardGit::from_command(real_git), &dir).unwrap();
    assert_eq!(place.tree, repository.0.join("wt"));
    assert_eq!(place.branch.as_deref(), Some("other"));
    assert_eq!(place.common_dir, repository.0.join("repo/.git"));
    assert_eq!(
        place.git_yard,
        repository.0.join("repo/.git/worktrees/wt/yard")
    );
}

#[test]
fn a_branch_is_cut_from_the_local_branch_its_reflog_names_and_from_none_otherwise() {
    let real = YardGit::from_command(real_git);
    let expected = [
        ("clean on main", None),
        ("a linked worktree", None),
        ("cut from a local branch", Some("feat")),
        ("cut from a revision that names no branch", None),
        ("cut from a branch, its reflog emptied", None),
    ];
    for (name, cut_from) in expected {
        let (_, build, _) = CASES.iter().find(|case| case.0 == name).unwrap();
        let repository = Scratch::new();
        let dir = build(&repository);
        let place = Place::locate(&real, &dir).unwrap();
        let branch = place.branch.unwrap();
        assert_eq!(
            real.cut_from(&dir, &branch).unwrap().as_deref(),
            cut_from,
            "{name}"
        );
    }
    let repository = Scratch::new();
    let (_, build, _) = CASES
        .iter()
        .find(|case| case.0 == "cut from a local branch")
        .unwrap();
    let dir = build(&repository);
    git(&dir, &["commit", "--quiet", "--allow-empty", "-m", "later"]);
    assert_eq!(
        real.cut_from(&dir, "topic").unwrap().as_deref(),
        Some("feat"),
        "a branch with commits since it was cut"
    );
    let repository = Scratch::new();
    let repo = repository.0.join("unborn");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "--quiet", "-b", "main"]);
    assert_eq!(
        real.cut_from(&repo, "main").unwrap(),
        None,
        "an unborn branch"
    );
}

#[test]
fn a_revision_that_names_nothing_holds_no_path() {
    let repository = Scratch::new();
    let dir = committed(&repository);
    let real = YardGit::from_command(real_git);
    for revision in ["no-such-branch", "-x", ""] {
        assert!(
            !real
                .holds(&dir, revision, ".yard/main/yardlog.yaml")
                .unwrap(),
            "{revision:?}"
        );
    }
}

#[test]
fn asking_git_changes_no_file_of_the_repository() {
    for (name, build, scenario) in CASES {
        let repository = Scratch::new();
        let dir = build(&repository);
        let before = repository.snapshot();
        let _ = learned(&YardGit::from_command(real_git), &dir);
        let plant = Scratch::new();
        let _ = learned(&simulated(&scenario(&repository.0), &plant.0), &dir);
        let after = repository.snapshot();
        let changed: Vec<&String> = before
            .keys()
            .chain(after.keys())
            .filter(|path| before.get(*path) != after.get(*path))
            .collect();
        assert!(changed.is_empty(), "{name}: changed {changed:?}");
    }
}

#[test]
fn a_scripted_git_error_is_reported_with_its_call_and_message() {
    let plant = Scratch::new();
    let scenario = GitScenario::clean_on("main").failing(
        software_plant::git::Call::Yard(software_plant::git::YardCall::Locate),
        128,
        "fatal: something broke\n",
    );
    let error = Place::locate(&simulated(&scenario, &plant.0), &plant.0).unwrap_err();
    assert!(
        matches!(&error, YardStoreError::Port(loft_ports::PortError::Git { call, message })
            if call == "rev-parse --show-toplevel --git-common-dir --git-path yard" && message == "fatal: something broke"),
        "{error}"
    );
}
