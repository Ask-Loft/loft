//! The clock contract: the clock port reads one instant per run, in whole seconds, within the
//! run; from the real clock when the Software Plant's clock simulator gives no time, and exactly
//! the simulator's time when it gives one.

use std::time::{SystemTime, UNIX_EPOCH};

use loftspec_store::StoreError;
use loftspec_store::clock::{Clock as _, SystemClock};
use software_plant::Clock;

fn seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// The instant the port reads for the next run of `clock`, with the window it must fall in: the
/// real clock's time around the read, or the simulator's time exactly.
fn read(clock: &Clock) -> (u64, u64, u64) {
    let epoch = clock.next_epoch();
    let before = seconds_now();
    let now = SystemClock::with_epoch(epoch.clone()).now().unwrap();
    let after = seconds_now();
    match epoch {
        Some(epoch) => {
            let epoch = epoch.parse().unwrap();
            (now, epoch, epoch)
        }
        None => (now, before, after),
    }
}

/// The scenarios of the contract, each run against the real clock and the simulator.
fn clocks() -> [(&'static str, Clock); 3] {
    [
        ("the real clock", Clock::system()),
        ("a fixed time", Clock::fixed(1_700_000_000)),
        ("a stepping time", Clock::stepping(0, 86_400)),
    ]
}

#[test]
fn each_run_reads_one_instant_within_the_run() {
    for (name, clock) in clocks() {
        let (now, earliest, latest) = read(&clock);
        assert!(
            (earliest..=latest).contains(&now),
            "{name}: {now} outside {earliest}..={latest}"
        );
    }
}

#[test]
fn a_later_run_reads_no_earlier_instant() {
    for (name, clock) in clocks() {
        let (first, ..) = read(&clock);
        let (second, ..) = read(&clock);
        assert!(second >= first, "{name}: {second} before {first}");
    }
}

#[test]
fn an_epoch_that_is_not_a_whole_number_of_seconds_is_refused_naming_it() {
    for epoch in ["", "-1", "1.5", "soon", "１２"] {
        let error = SystemClock::with_epoch(Some(epoch)).now().unwrap_err();
        assert!(
            matches!(&error, StoreError::Epoch(value) if value == epoch),
            "{epoch:?}"
        );
        assert!(error.to_string().contains("SOURCE_DATE_EPOCH"), "{error}");
    }
}
