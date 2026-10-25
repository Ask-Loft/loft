//! The clock simulator gives each run the time its scenario names.

use std::process::Command;

use software_plant::Clock;
use software_plant::clock::{EPOCH_ENV, PASSING_ENV};

fn epoch_of(command: &Command) -> Option<String> {
    command
        .get_envs()
        .find(|(name, _)| *name == EPOCH_ENV)
        .and_then(|(_, value)| value.map(|value| value.to_string_lossy().into_owned()))
}

#[test]
fn a_fixed_scenario_gives_every_run_the_same_epoch() {
    let clock = Clock::fixed(1_700_000_000);
    for _ in 0..3 {
        let mut command = Command::new("loft");
        clock.apply(&mut command);
        assert_eq!(epoch_of(&command).as_deref(), Some("1700000000"));
    }
}

#[test]
fn a_stepping_scenario_advances_the_epoch_by_its_step_on_each_run() {
    let clock = Clock::stepping(100, 60);
    let epochs: Vec<_> = (0..3).map(|_| clock.next_epoch()).collect();
    assert_eq!(
        epochs,
        [Some("100".into()), Some("160".into()), Some("220".into())]
    );
}

#[test]
fn a_run_outside_any_scenario_leaves_the_epoch_unset() {
    let mut command = Command::new("loft");
    command.env(EPOCH_ENV, "5");
    Clock::system().apply(&mut command);
    assert_eq!(epoch_of(&command), None);
}

fn passing_of(command: &Command) -> Option<String> {
    command
        .get_envs()
        .find(|(name, _)| *name == PASSING_ENV)
        .and_then(|(_, value)| value.map(|value| value.to_string_lossy().into_owned()))
}

#[test]
fn a_scenario_whose_time_passes_sets_loft_clock_and_any_other_leaves_it_unset() {
    for (clock, expected) in [
        (Clock::fixed(100).passing(), Some("passing")),
        (Clock::stepping(100, 60).passing(), Some("passing")),
        (Clock::fixed(100), None),
        (Clock::system(), None),
    ] {
        for _ in 0..2 {
            let mut command = Command::new("loft");
            command.env(PASSING_ENV, "passing");
            clock.apply(&mut command);
            assert_eq!(passing_of(&command).as_deref(), expected);
        }
    }
    let mut command = Command::new("loft");
    Clock::fixed(100).passing().apply(&mut command);
    assert_eq!(epoch_of(&command).as_deref(), Some("100"));
}
