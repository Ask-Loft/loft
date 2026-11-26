//! Yard Server's answer to a batch of versions, `yardlog-answer/1`: each version's key and its
//! outcome, read so that the sending settles each entry by it and leaves the rest where it is.

use yardlog::batch_answer::{Outcome, read};

#[test]
fn an_answer_gives_each_version_its_outcome_and_code() {
    let answer = read(
        br#"{"format":"yardlog-answer/1","outcomes":[
            {"version":"a","outcome":"stored"},
            {"version":"b","outcome":"held"},
            {"version":"c","outcome":"conflict"},
            {"version":"d","outcome":"refused","code":"invalid_record"},
            {"version":"e","outcome":"later","code":"unknown_report_version"}
        ]}"#,
    )
    .unwrap();
    assert_eq!(answer.get("a"), Some(&Outcome::Stored));
    assert_eq!(answer.get("b"), Some(&Outcome::Held));
    assert_eq!(answer.get("c"), Some(&Outcome::Conflict));
    assert_eq!(
        answer.get("d"),
        Some(&Outcome::Refused("invalid_record".to_owned()))
    );
    assert_eq!(
        answer.get("e"),
        Some(&Outcome::Later("unknown_report_version".to_owned()))
    );
    assert_eq!(answer.get("f"), None);
}

#[test]
fn an_outcome_this_loft_does_not_know_or_one_without_its_code_is_unknown() {
    let answer = read(
        br#"{"format":"yardlog-answer/1","outcomes":[
            {"version":"a","outcome":"maybe"},
            {"version":"b","outcome":"refused"},
            {"version":"c","outcome":"later"}
        ]}"#,
    )
    .unwrap();
    for version in ["a", "b", "c"] {
        assert_eq!(answer.get(version), Some(&Outcome::Unknown), "{version}");
    }
}

#[test]
fn an_answer_that_is_not_json_or_of_another_format_does_not_read() {
    for body in [
        &b"not json"[..],
        br#"{"format":"yardlog-answer/2","outcomes":[]}"#,
        br#"{"format":"yardlog-answer/1"}"#,
        br#"{"format":"yardlog-answer/1","outcomes":{}}"#,
        b"[]",
    ] {
        assert_eq!(read(body), None, "{}", String::from_utf8_lossy(body));
    }
}

#[test]
fn an_outcome_without_a_version_s_key_is_passed_over() {
    let answer = read(
        br#"{"format":"yardlog-answer/1","outcomes":[{"outcome":"stored"},{"version":"a","outcome":"held"}]}"#,
    )
    .unwrap();
    assert_eq!(answer.len(), 1);
    assert_eq!(answer.get("a"), Some(&Outcome::Held));
}
