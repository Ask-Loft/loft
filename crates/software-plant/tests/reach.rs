//! A simulator reaches the binary under test only through its search path or its environment:
//! applied to a command, it sets no argument and no working directory, and of the environment
//! only the search path and the variables it documents.

use std::ffi::OsStr;
use std::fs;
use std::process::{self, Command};

use software_plant::clock::{EPOCH_ENV, PASSING_ENV};
use software_plant::git::SCENARIO_ENV;
use software_plant::{Clock, GitScenario};

/// The names of the environment variables `command` sets or removes.
fn environment(command: &Command) -> Vec<&str> {
    let mut names: Vec<&str> = command
        .get_envs()
        .map(|(name, _)| name.to_str().unwrap())
        .collect();
    names.sort_unstable();
    names
}

fn assert_reaches_only_by_environment(command: &Command) {
    assert_eq!(command.get_program(), OsStr::new("loft"));
    assert_eq!(command.get_args().count(), 0, "{command:?}");
    assert_eq!(command.get_current_dir(), None, "{command:?}");
}

#[test]
fn the_git_simulator_sets_only_the_search_path_and_its_scenario() {
    let dir = std::env::temp_dir().join(format!("software-plant-reach-{}", process::id()));
    fs::create_dir_all(&dir).unwrap();
    let installed = GitScenario::clean_on("main").install(&dir);
    let mut command = Command::new("loft");
    installed.apply(&mut command);
    assert_reaches_only_by_environment(&command);
    let mut expected = vec!["PATH", SCENARIO_ENV];
    expected.sort_unstable();
    assert_eq!(environment(&command), expected);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_clock_simulator_sets_only_its_epoch_and_whether_time_passes() {
    for clock in [
        Clock::fixed(1_700_000_000),
        Clock::stepping(100, 60),
        Clock::system(),
        Clock::fixed(1_700_000_000).passing(),
    ] {
        let mut command = Command::new("loft");
        clock.apply(&mut command);
        assert_reaches_only_by_environment(&command);
        assert_eq!(environment(&command), [PASSING_ENV, EPOCH_ENV], "{clock:?}");
    }
}
