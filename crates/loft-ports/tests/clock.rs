//! The clock port: whole seconds since the Unix epoch, from `SOURCE_DATE_EPOCH` when it is set.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use loft_ports::PortError;
use loft_ports::clock::{Clock as _, SystemClock};

#[test]
fn a_given_epoch_is_the_time() {
    assert_eq!(
        SystemClock::with_epoch(Some("1700000000")).now().unwrap(),
        1_700_000_000
    );
}

#[test]
fn an_epoch_that_is_not_whole_seconds_is_refused() {
    for epoch in ["", "1.5", "-1", "soon"] {
        let error = SystemClock::with_epoch(Some(epoch)).now().unwrap_err();
        assert!(matches!(error, PortError::Epoch(_)), "{epoch:?}: {error}");
    }
}

#[test]
fn without_an_epoch_the_time_is_the_system_s() {
    let seconds = || {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    };
    let before = seconds();
    let now = SystemClock::with_epoch(None::<&str>).now().unwrap();
    assert!((before..=seconds()).contains(&now));
}

/// The scenarios of the waiting contract, each run against the real clock and the simulator: the
/// port's clock for a run of each, as the run's environment would make it.
fn waiting_clocks() -> [(&'static str, SystemClock); 2] {
    let made = |scenario: &software_plant::Clock| {
        SystemClock::with_epoch(scenario.next_epoch()).passing_if(scenario.is_passing())
    };
    [
        ("the real clock", made(&software_plant::Clock::system())),
        (
            "a passing time",
            made(&software_plant::Clock::fixed(1_700_000_000).passing()),
        ),
    ]
}

#[test]
fn a_wait_moves_the_time_on_by_at_least_its_length_on_the_real_clock_as_on_the_simulator() {
    for (name, clock) in waiting_clocks() {
        let (before, elapsed) = (clock.now().unwrap(), clock.elapsed());
        clock.wait(Duration::from_secs(1));
        assert!(
            clock.now().unwrap() > before,
            "{name}: the time did not move"
        );
        assert!(
            clock.elapsed() >= elapsed + Duration::from_secs(1),
            "{name}: less than the wait passed"
        );
    }
}

#[test]
fn where_time_passes_a_wait_returns_at_once_and_moves_the_time_on_by_its_length() {
    let clock = SystemClock::with_epoch(Some("1700000000")).passing_if(true);
    let started = Instant::now();
    clock.wait(Duration::from_secs(540));
    clock.wait(Duration::from_millis(500));
    clock.wait(Duration::from_millis(500));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(clock.elapsed(), Duration::from_secs(541));
    assert_eq!(clock.now().unwrap(), 1_700_000_541);
}

#[test]
fn where_time_passes_the_time_moves_only_as_the_clock_waits() {
    let clock = SystemClock::with_epoch(Some("1700000000")).passing_if(true);
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(clock.now().unwrap(), 1_700_000_000);
    assert_eq!(clock.elapsed(), Duration::ZERO);
    let without_epoch = SystemClock::with_epoch(None::<&str>).passing_if(true);
    let first = without_epoch.now().unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(without_epoch.now().unwrap(), first);
}

#[test]
fn a_clone_of_a_clock_is_the_same_clock() {
    let clock = SystemClock::with_epoch(Some("100")).passing_if(true);
    clock.clone().wait(Duration::from_secs(60));
    assert_eq!(clock.now().unwrap(), 160);
}
