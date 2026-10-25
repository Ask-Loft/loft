//! `loft yard continue`: a session that continues one that ended records it once, in the
//! machine's home, with no repository and no `.yard/`.

use serde_json::json;

use crate::yard::{Yard, document};

#[test]
fn loft_yard_continue_needs_no_git_repository() {
    let yard = Yard::new();
    // No `git` is on the search path: a command that asked git anything would fail.
    let args = [
        "yard",
        "continue",
        "--session",
        "s2",
        "--from",
        "s1",
        "--format",
        "json",
    ];
    let output = yard.scratch.loft_with(&args, None, &yard.clock);
    assert_eq!(output.status.code(), Some(0));
    let recorded = document("continue", &output);
    assert_eq!(recorded["outcome"], "recorded");
    assert_eq!(recorded["changed"], true);
    assert_eq!(yard.home_file("continuations.json"), json!({ "s2": "s1" }));
}

#[test]
fn recording_a_continuation_already_recorded_changes_no_file() {
    let yard = Yard::new();
    yard.run(&["continue", "--session", "s2", "--from", "s1"]);
    let path = yard.scratch.home().join(".yard").join("continuations.json");
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    let again = document(
        "continue",
        &yard.run(&["continue", "--session", "s2", "--from", "s1"]),
    );
    assert_eq!(again["changed"], false);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before
    );
}

#[test]
fn loft_yard_continue_without_session_or_from_exits_2_and_names_the_one_missing() {
    let yard = Yard::new();
    for (args, missing) in [
        (&["continue", "--from", "s1"][..], "--session"),
        (&["continue", "--session", "s2"], "--from"),
    ] {
        let output = yard.run(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(missing), "{stderr}");
    }
}
