//! Two publishes that meet (F-58) under real git: Alice publishes in the
//! origin, Bob publishes in his clone and pulls by merge or by rebase, `loft publish` settles the
//! stop, and Bob finishes with the command it prints. Every scenario that finishes ends with
//! `loft check --fail-on warning` and `loft verify` passing.

use std::fs;
use std::process::{Command, Output};

use crate::common::{EPOCH, Scratch, stderr, stdout};
use crate::repo::Repo;
use crate::snapshot::snapshot;
use software_plant::Clock;
use software_plant::real_git::{isolate, real_git};

const SHARE: &str = "products/reading-list/sharing/share-a-list.yaml";
const REMOVE: &str = "products/reading-list/shelf/remove-a-book.yaml";

/// Runs `loft` in `repo` with real git on its search path, at [`EPOCH`].
fn loft(repo: &Repo, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_loft"));
    isolate(&mut command).current_dir(&repo.0.0);
    Clock::fixed(EPOCH).apply(&mut command);
    command.args(args).output().unwrap()
}

fn loft_ok(repo: &Repo, args: &[&str]) -> String {
    let output = loft(repo, args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "loft {args:?}: {}",
        stderr(&output)
    );
    stdout(&output)
}

/// Commits the changes in `repo`, where it has any.
fn commit_changes(repo: &Repo, message: &str) {
    let status = repo.git_ok(&["status", "--porcelain"]);
    if !status.stdout.is_empty() {
        repo.commit(message);
    }
}

/// Commits the changes in `repo` as `message`, publishes, and commits what the publish wrote.
fn published(repo: &Repo, message: &str) {
    commit_changes(repo, message);
    loft_ok(repo, &["publish"]);
    commit_changes(repo, "publish");
}

/// Alice's repository, the origin, at its first publish, and Bob's clone of it.
fn clones() -> (Repo, Repo) {
    let alice = Repo::new();
    published(&alice, "publish 1");
    let bob = Repo(Scratch::with_reading_list());
    fs::remove_dir_all(&bob.0.0).unwrap();
    let cloned = real_git()
        .args(["clone", "--quiet"])
        .arg(&alice.0.0)
        .arg(&bob.0.0)
        .output()
        .unwrap();
    assert!(cloned.status.success(), "{}", stderr(&cloned));
    (alice, bob)
}

/// A Requirement with one criterion, keyed `…a<n>` and `…b<n>`, appended to the Feature `path`.
fn add_requirement(repo: &Repo, path: &str, n: u32, shall: &str) {
    let text = format!(
        "  - key: 00000000-0000-4000-8000-0000000000a{n}
    statement:
      form: ubiquitous
      system: \"The shelf\"
      shall: \"{shall}\"
    criteria:
      - key: 00000000-0000-4000-8000-0000000000b{n}
        name: \"The shelf does so.\"
"
    );
    let mut current = repo.text(path);
    current.push_str(&text);
    fs::write(repo.0.file(path), current).unwrap();
}

/// Alice and Bob each add a Requirement and publish, so each gives `RQ-7` and `AC-8` to a
/// different node; Alice's commit is in the origin, Bob's in his clone.
fn two_publishes() -> (Repo, Repo) {
    let (alice, bob) = clones();
    add_requirement(&alice, SHARE, 1, "export a list.");
    published(&alice, "alice publishes");
    add_requirement(&bob, REMOVE, 2, "import a list.");
    published(&bob, "bob publishes");
    (alice, bob)
}

/// Pulls with `how`, which stops on a conflict.
fn pull_stops(repo: &Repo, how: &str) {
    let output = repo.git(&["pull", "--quiet", how]);
    assert!(!output.status.success(), "git pull {how} stops");
}

/// Settles the stop with `loft publish`, runs the command it prints, and gives what it printed.
fn settle(repo: &Repo) -> String {
    let printed = loft_ok(repo, &["publish"]);
    let finish = printed
        .lines()
        .find_map(|line| line.strip_prefix("to finish: "))
        .expect("the finishing command");
    let continued = finish.strip_prefix("git add -A && git ").unwrap();
    repo.git_ok(&["add", "-A"]);
    let args: Vec<&str> = continued.split(' ').collect();
    let output = repo.git(&args);
    printed + &format!("finished: {}\n", output.status.success())
}

/// `loft check --fail-on warning` and `loft verify`, neither of which reads git, pass.
fn checks_and_verifies(repo: &Repo) {
    for args in [&["check", "--fail-on", "warning"][..], &["verify"]] {
        let output = repo.0.loft(args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "loft {args:?}: {}",
            stdout(&output) + &stderr(&output)
        );
    }
}

#[test]
fn a_pull_by_merge_settles_in_one_publish_and_the_merge_commit_checks_and_verifies() {
    let (_alice, bob) = two_publishes();
    pull_stops(&bob, "--no-rebase");
    let printed = settle(&bob);
    assert!(
        printed.contains(
            "withdrew RQ-7 from 00000000-0000-4000-8000-0000000000a2, which now holds RQ-8"
        ),
        "{printed}"
    );
    assert!(
        printed.contains("to finish: git add -A && git commit --no-edit\nfinished: true"),
        "{printed}"
    );
    let labels = bob.text("labels.yaml");
    assert!(
        labels.contains("  - key: 00000000-0000-4000-8000-0000000000a1\n    label: RQ-7\n"),
        "{labels}"
    );
    checks_and_verifies(&bob);
    assert!(loft_ok(&bob, &["publish"]).starts_with("nothing changed"));
}

#[test]
fn a_pull_by_rebase_settles_and_the_next_publish_rewrites_a_later_citation() {
    let (_alice, bob) = two_publishes();
    bob.0.edit(
        REMOVE,
        "name: \"Remove a book\"\nrealizes:\n  - 00000000-0000-4000-8000-00000000003c\n",
        "name: \"Remove a book\"\nbody: \"Imports follow RQ-7.\"\nrealizes:\n  - 00000000-0000-4000-8000-00000000003c\nmentions:\n  - 00000000-0000-4000-8000-0000000000a2\n",
    );
    bob.commit("bob cites his Requirement");
    pull_stops(&bob, "--rebase");
    let printed = settle(&bob);
    assert!(
        printed.contains("to finish: git add -A && git rebase --continue\nfinished: true"),
        "{printed}"
    );
    assert!(bob.text(REMOVE).contains("Imports follow RQ-7."));
    let republished = loft_ok(&bob, &["publish"]);
    assert!(
        republished.contains("rewrote RQ-7 as RQ-8 in 00000000-0000-4000-8000-00000000001f"),
        "{republished}"
    );
    assert!(bob.text(REMOVE).contains("Imports follow RQ-8."));
    commit_changes(&bob, "publish");
    checks_and_verifies(&bob);
}

#[test]
fn a_rebase_of_two_publish_commits_stops_twice_and_settles_each() {
    let (_alice, bob) = two_publishes();
    add_requirement(&bob, REMOVE, 3, "merge two lists.");
    published(&bob, "bob publishes again");
    pull_stops(&bob, "--rebase");
    let first = settle(&bob);
    assert!(
        first.contains("finished: false"),
        "the rebase stops again: {first}"
    );
    let second = settle(&bob);
    assert!(second.contains("finished: true"), "{second}");
    checks_and_verifies(&bob);
}

/// Alice and Bob each reword a statement and publish, so only the lock conflicts.
#[test]
fn a_conflict_on_the_lock_alone_is_settled_by_a_publish() {
    let (alice, bob) = clones();
    alice.0.edit(
        SHARE,
        "refuse edits made through the link.",
        "refuse every edit made through the link.",
    );
    published(&alice, "alice publishes");
    bob.0.edit(
        REMOVE,
        "report that the book is absent.",
        "report the book as absent.",
    );
    published(&bob, "bob publishes");
    pull_stops(&bob, "--no-rebase");
    let printed = settle(&bob);
    assert!(!printed.contains("withdrew"), "{printed}");
    assert!(printed.contains("published sequence 3"), "{printed}");
    checks_and_verifies(&bob);
}

fn refused(repo: &Repo, naming: &str) {
    let before = snapshot(&repo.0.0);
    let output = loft(repo, &["publish"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains(naming), "{}", stderr(&output));
    assert_eq!(snapshot(&repo.0.0), before, "nothing is written");
}

#[test]
fn a_merge_into_a_branch_other_than_the_publish_branch_is_refused() {
    let (_alice, bob) = two_publishes();
    bob.git_ok(&["checkout", "--quiet", "-b", "topic"]);
    let output = bob.git(&["pull", "--quiet", "--no-rebase", "origin", "main"]);
    assert!(!output.status.success());
    refused(&bob, "`topic`");
}

/// Writes `src/main.rs` beside `spec/`.
fn code(repo: &Repo, text: &str) {
    fs::create_dir_all(repo.0.0.join("src")).unwrap();
    fs::write(repo.0.0.join("src/main.rs"), text).unwrap();
}

#[test]
fn an_unmerged_code_file_is_refused_by_name() {
    let (alice, bob) = clones();
    add_requirement(&alice, SHARE, 1, "export a list.");
    code(&alice, "fn main() {}\n");
    published(&alice, "alice publishes");
    add_requirement(&bob, REMOVE, 2, "import a list.");
    code(&bob, "fn main() { println!(); }\n");
    published(&bob, "bob publishes");
    pull_stops(&bob, "--no-rebase");
    refused(&bob, "src/main.rs");
}

#[test]
fn a_pull_git_could_not_run_the_driver_for_is_refused_and_recovered_by_checkout_merge() {
    let (_alice, bob) = two_publishes();
    let broken = "merge.loft.driver=no-such-loft merge-driver %O %A %B %P";
    let output = bob.git(&["-c", broken, "pull", "--quiet", "--no-rebase"]);
    assert!(!output.status.success(), "the pull stops");
    assert!(!bob.text("labels.yaml").contains("conflicts:"));
    refused(&bob, "git checkout -m spec/labels.yaml");
    bob.git_ok(&["checkout", "-m", "spec/labels.yaml"]);
    assert!(bob.text("labels.yaml").contains("conflicts:"));
    let printed = settle(&bob);
    assert!(printed.contains("withdrew RQ-7"), "{printed}");
    assert!(printed.contains("finished: true"), "{printed}");
    checks_and_verifies(&bob);
}
