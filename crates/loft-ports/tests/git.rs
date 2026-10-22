//! The git runner: each call runs `git -C <directory>` with git's optional locks off, a git that
//! cannot be run is reported as unavailable, and a failed call by its call and message. The
//! Software Plant's simulator stands in for git.

use std::process::Command;

use crate::common::Scratch;
use loft_ports::PortError;
use loft_ports::git::{Git, first_line, succeeded};
use software_plant::GitScenario;

fn simulated(scenario: &GitScenario, plant: &Scratch) -> (Git, software_plant::Installed) {
    let installed = scenario.install(&plant.0);
    let applied = installed.clone();
    let git = Git::from_command(move || {
        let mut command = Command::new("git");
        applied.apply(&mut command);
        command
    });
    (git, installed)
}

#[test]
fn a_call_runs_in_the_directory_it_names() {
    let plant = Scratch::new();
    let (git, _) = simulated(&GitScenario::clean_on("main").below("sub/"), &plant);
    let call = ["rev-parse", "--show-prefix"];
    let output = git.run(&plant.0, &call).unwrap();
    assert_eq!(first_line(&succeeded(&call, output).unwrap()), "sub/");
}

#[test]
fn a_call_takes_none_of_git_s_optional_locks() {
    let plant = Scratch::new();
    let (git, installed) = simulated(&GitScenario::clean_on("main"), &plant);
    let call = [
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--",
        "spec",
    ];
    git.run(&plant.0, &call).unwrap();
    assert_eq!(installed.locks(), Vec::<String>::new());
}

#[test]
fn a_git_that_cannot_be_run_is_reported_as_unavailable() {
    let plant = Scratch::new();
    let missing = plant.0.join("no-such-git");
    let git = Git::from_command(move || Command::new(&missing));
    let error = git.run(&plant.0, &["status"]).unwrap_err();
    assert!(matches!(error, PortError::GitUnavailable(_)), "{error}");
}

#[test]
fn a_failed_call_is_reported_with_its_call_and_message() {
    let plant = Scratch::new();
    let scenario = GitScenario::clean_on("main").failing(
        software_plant::git::Call::ShowPrefix,
        128,
        "fatal: something broke\n",
    );
    let (git, _) = simulated(&scenario, &plant);
    let call = ["rev-parse", "--show-prefix"];
    let error = succeeded(&call, git.run(&plant.0, &call).unwrap()).unwrap_err();
    assert!(
        matches!(&error, PortError::Git { call, message }
            if call == "rev-parse --show-prefix" && message == "fatal: something broke"),
        "{error}"
    );
}

#[test]
fn a_call_with_input_reads_it_on_its_standard_input() {
    let plant = Scratch::new();
    let (git, _) = simulated(&GitScenario::clean_on("main"), &plant);
    let output = git
        .run_with_input(&plant.0, &["cat-file", "--batch"], b"0123\n")
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0123 missing\n");
}
