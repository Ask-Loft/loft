//! The clock port: whole seconds since the Unix epoch, from `SOURCE_DATE_EPOCH` when it is set.

use std::time::{SystemTime, UNIX_EPOCH};

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
