//! The git simulator answers each call Loft makes as its scenario scripts it, fails as git
//! fails where the scenario says so, and refuses every other call.

use std::process::Command;

use software_plant::GitScenario;
use software_plant::git::{Call, NOT_A_REPOSITORY, Reply, UNSCRIPTED};

fn args(line: &str) -> Vec<String> {
    line.split(' ').map(str::to_owned).collect()
}

const STATUS: &str = "-C /work status --porcelain=v1 -z --untracked-files=all -- spec";

fn answer(scenario: &GitScenario, line: &str) -> Reply {
    scenario.answer(&args(line))
}

#[test]
fn the_prefix_is_answered_as_rev_parse_prints_it() {
    let top = GitScenario::clean_on("main");
    assert_eq!(answer(&top, "rev-parse --show-prefix").stdout, b"\n");
    let below = top.below("sub/");
    assert_eq!(
        answer(&below, "-C /work rev-parse --show-prefix").stdout,
        b"sub/\n"
    );
}

#[test]
fn the_branch_is_answered_as_symbolic_ref_prints_it() {
    let reply = answer(
        &GitScenario::clean_on("main"),
        "symbolic-ref --quiet --short HEAD",
    );
    assert_eq!((reply.code, reply.stdout), (0, b"main\n".to_vec()));
}

#[test]
fn a_detached_head_exits_1_and_prints_nothing() {
    let reply = answer(
        &GitScenario::clean_on("main").detached(),
        "symbolic-ref --quiet --short HEAD",
    );
    assert_eq!(
        reply,
        Reply {
            code: 1,
            stdout: Vec::new(),
            stderr: String::new()
        }
    );
}

#[test]
fn the_status_is_answered_as_porcelain_v1_prints_it() {
    let scenario = GitScenario::clean_on("main")
        .with("??", "spec/x/y")
        .with(" M", "spec/a")
        .with("D ", "spec/z")
        .with("??", "spec/new")
        .renamed("spec/c", "spec/c2")
        .with(" M", "README.md");
    let reply = answer(&scenario, STATUS);
    assert_eq!(
        reply.stdout,
        b" M spec/a\0R  spec/c2\0spec/c\0D  spec/z\0?? spec/new\0?? spec/x/y\0".to_vec()
    );
}

#[test]
fn the_status_takes_its_pathspec_from_the_working_directory() {
    let scenario = GitScenario::clean_on("main")
        .below("docs/")
        .with(" M", "docs/spec/a")
        .with(" M", "spec/b")
        .with(" M", "docs/specs/c");
    assert_eq!(
        answer(&scenario, STATUS).stdout,
        b" M docs/spec/a\0".to_vec()
    );
}

#[test]
fn outside_a_repository_each_call_exits_128_with_gits_message() {
    let scenario = GitScenario::outside_repository();
    for line in [
        "rev-parse --show-prefix",
        "symbolic-ref --quiet --short HEAD",
        STATUS,
    ] {
        let reply = answer(&scenario, line);
        assert_eq!(
            (reply.code, reply.stderr.as_str()),
            (128, NOT_A_REPOSITORY),
            "{line}"
        );
    }
}

#[test]
fn a_scripted_error_exits_with_its_code_and_message() {
    let scenario =
        GitScenario::clean_on("main").failing(Call::Status, 128, "fatal: index broken\n");
    let reply = answer(&scenario, STATUS);
    assert_eq!(
        (reply.code, reply.stderr.as_str()),
        (128, "fatal: index broken\n")
    );
    assert_eq!(answer(&scenario, "rev-parse --show-prefix").code, 0);
}

#[test]
fn an_unscripted_call_names_each_argument_and_exits_2() {
    let reply = answer(&GitScenario::clean_on("main"), "log --oneline -1");
    assert_eq!(reply.code, UNSCRIPTED);
    assert_eq!(UNSCRIPTED, 2);
    assert!(
        reply.stderr.contains("git log --oneline -1"),
        "{}",
        reply.stderr
    );
}

#[test]
fn the_executable_reads_its_scenario_from_the_environment() {
    let dir = std::env::temp_dir().join(format!("software-plant-git-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let installed = GitScenario::clean_on("trunk").install(&dir);
    let output = installed
        .apply(&mut Command::new("git"))
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(output.stdout, b"trunk\n");
}

#[test]
fn the_executable_without_a_scenario_exits_2() {
    let output = Command::new(software_plant::git_executable())
        .env_remove(software_plant::git::SCENARIO_ENV)
        .args(["rev-parse", "--show-prefix"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(UNSCRIPTED));
}

#[test]
fn a_scenario_round_trips_through_its_file() {
    let dir = std::env::temp_dir().join(format!("software-plant-file-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let scenario = GitScenario::clean_on("main")
        .below("sub/")
        .renamed("a", "b")
        .failing(Call::ShowPrefix, 1, "x");
    let path = dir.join("scenario.json");
    scenario.write(&path);
    let read = GitScenario::read(&path);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(read.unwrap(), scenario);
}
