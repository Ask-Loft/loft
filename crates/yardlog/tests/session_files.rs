//! Two copies of a file one session writes alone, its tokens or its activity, merged as git's
//! merge driver merges them: one copy taken whole, the one whose last event ended later.

use yardlog::merge::{Taken, take_session_file};

const NAME: &str = "tokens/0b6b3f8e-7c55-4f6c-9d3a-5a2f1f0c8e11.yaml";

/// A session's file whose last event ended at `last`, with `requests` counted.
fn file(last: &str, requests: u32) -> String {
    format!(
        "session: 0b6b3f8e-7c55-4f6c-9d3a-5a2f1f0c8e11\nlastEnded: {last}\nrequests: {requests}\n"
    )
}

#[test]
fn the_copy_whose_last_event_ended_later_is_taken() {
    let ours = file("2026-01-01T00:01:00Z", 1);
    let theirs = file("2026-01-01T00:02:00Z", 2);
    let taken = take_session_file(NAME, ours.as_bytes(), theirs.as_bytes()).unwrap();
    assert_eq!(taken, Taken::Theirs);
    let taken = take_session_file(NAME, theirs.as_bytes(), ours.as_bytes()).unwrap();
    assert_eq!(taken, Taken::Ours);
}

#[test]
fn of_two_copies_whose_last_events_ended_at_one_time_this_copy_is_taken() {
    let ours = file("2026-01-01T00:01:00Z", 1);
    let theirs = file("2026-01-01T00:01:00Z", 2);
    let taken = take_session_file(NAME, ours.as_bytes(), theirs.as_bytes()).unwrap();
    assert_eq!(taken, Taken::Ours);
}

#[test]
fn a_copy_that_holds_no_ended_event_is_taken_only_where_the_other_holds_none_either() {
    let empty = "session: 0b6b3f8e-7c55-4f6c-9d3a-5a2f1f0c8e11\n";
    let ended = file("2026-01-01T00:01:00Z", 1);
    let taken = take_session_file(NAME, empty.as_bytes(), ended.as_bytes()).unwrap();
    assert_eq!(taken, Taken::Theirs);
    let taken = take_session_file(NAME, ended.as_bytes(), empty.as_bytes()).unwrap();
    assert_eq!(taken, Taken::Ours);
}

#[test]
fn a_copy_that_does_not_read_is_named_with_its_line_and_column() {
    let ours = file("2026-01-01T00:01:00Z", 1);
    let theirs = "session: 0b6b3f8e-7c55-4f6c-9d3a-5a2f1f0c8e11\nlastEnded: yesterday\n";
    let (which, error) = take_session_file(NAME, ours.as_bytes(), theirs.as_bytes()).unwrap_err();
    assert_eq!(which, "theirs");
    assert_eq!(error.file, NAME);
    let position = error.error.position.expect("a position");
    assert_eq!((position.line, position.column), (2, 12));
    let (which, _) = take_session_file(NAME, b"- not\n- a mapping\n", ours.as_bytes()).unwrap_err();
    assert_eq!(which, "ours");
}
