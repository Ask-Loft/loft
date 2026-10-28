use super::sums;
use serde_json::{Value, json};

fn counts(input: u64, cache_read: u64, cache_write: u64, output: u64) -> Value {
    json!({ "input": input, "cacheRead": cache_read, "cacheWrite": cache_write, "output": output })
}

/// `counts` with the fields of `more` beside them.
fn with(counts: Value, more: Value) -> Value {
    let mut joined = counts;
    let Value::Object(more) = more else {
        unreachable!("the fields beside the counts are a mapping")
    };
    for (name, value) in more {
        joined[name] = value;
    }
    joined
}

/// Two Claude Code sessions' totals files over the stage `shape`.
fn two_sessions() -> Vec<Value> {
    vec![
        json!({
            "host": "claude-code",
            "session": "s1",
            "firstEnded": "2026-01-01T10:00:00Z",
            "lastEnded": "2026-01-01T10:30:00Z",
            "tokens": counts(100, 1000, 10, 50),
            "noStage": counts(20, 0, 0, 5),
            "stages": [with(counts(80, 1000, 10, 45), json!({
                "stage": "shape",
                "agents": [
                    with(counts(50, 900, 10, 35), json!({ "agent": "main" })),
                    with(counts(30, 100, 0, 10), json!({ "agent": "a1", "type": "Explore" })),
                ],
            }))],
            "agents": [
                with(counts(70, 900, 10, 40), json!({ "agent": "main", "model": "opus" })),
                with(counts(30, 100, 0, 10), json!({ "agent": "a1", "type": "Explore", "model": "haiku" })),
            ],
            "submissions": [with(counts(10, 0, 0, 1), json!({ "submission": "sub" }))],
        }),
        json!({
            "host": "claude-code",
            "session": "s2",
            "firstEnded": "2026-01-01T09:00:00Z",
            "lastEnded": "2026-01-01T09:10:00Z",
            "tokens": counts(1, 2, 3, 4),
            "stages": [with(counts(1, 2, 3, 4), json!({ "stage": "shape" }))],
            "agents": [with(counts(1, 2, 3, 4), json!({ "agent": "main", "model": "opus" }))],
        }),
    ]
}

/// A Codex totals file whose requests each wrote nothing to the cache.
fn codex_totals() -> Value {
    let tokens = counts(10, 100, 0, 2);
    json!({
        "host": "codex",
        "session": "codex",
        "tokens": tokens,
        "noStage": counts(0, 0, 0, 0),
        "stages": [with(tokens.clone(), json!({
            "stage": "shape",
            "agents": [with(tokens.clone(), json!({ "agent": "child", "type": "review" }))],
        }))],
        "agents": [with(tokens.clone(), json!({ "agent": "child", "type": "review", "model": "fixture" }))],
        "submissions": [with(tokens, json!({ "submission": "sub" }))],
    })
}

/// A totals file of another host's session `session` holding `tokens` in all.
fn other(session: &str, tokens: &Value) -> Value {
    json!({ "host": "other-agent", "session": session, "tokens": tokens })
}

/// Every sum of `sums`, as the page reads them.
fn every_sum(sums: &Value) -> Vec<&Value> {
    let stage = &sums["stages"][0];
    vec![
        &sums["all"],
        &sums["noStage"],
        &stage["tokens"],
        &stage["agents"][0]["tokens"],
        &sums["agents"][0]["tokens"],
        &sums["sessions"][0]["tokens"],
        &sums["submissions"][0]["tokens"],
    ]
}

#[test]
fn the_four_counts_in_all_are_summed_over_every_totals_file() {
    let sums = sums(&two_sessions(), &[]);
    assert_eq!(sums["all"], counts(101, 1002, 13, 54));
    assert_eq!(sums["counted"], true);
    assert_eq!(sums["partial"], false);
}

#[test]
fn each_stage_has_its_counts_and_its_subagents_within_them_and_those_under_no_stage_apart() {
    let sums = sums(&two_sessions(), &[]);
    assert_eq!(
        sums["stages"],
        json!([{
            "stage": "shape",
            "tokens": counts(81, 1002, 13, 49),
            "agents": [{ "type": "Explore", "tokens": counts(30, 100, 0, 10) }],
        }])
    );
    // The second session keeps nothing under no stage: a measured zero of each counter.
    assert_eq!(sums["noStage"], counts(20, 0, 0, 5));
}

#[test]
fn each_session_has_its_counts_and_when_its_requests_ended_the_earliest_first() {
    let sums = sums(&two_sessions(), &[]);
    let sessions = sums["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0]["session"], "s2");
    assert_eq!(sessions[1]["session"], "s1");
    assert_eq!(sessions[1]["host"], "claude-code");
    assert_eq!(sessions[1]["firstEnded"], "2026-01-01T10:00:00Z");
    assert_eq!(sessions[1]["lastEnded"], "2026-01-01T10:30:00Z");
    assert_eq!(sessions[1]["tokens"], counts(100, 1000, 10, 50));
}

#[test]
fn the_main_agent_and_each_type_of_subagent_have_their_counts_for_each_model() {
    let sums = sums(&two_sessions(), &[]);
    assert_eq!(
        sums["agents"],
        json!([
            { "who": "main agent", "model": "opus", "tokens": counts(71, 902, 13, 44) },
            { "who": "Explore", "model": "haiku", "tokens": counts(30, 100, 0, 10) },
        ])
    );
}

#[test]
fn an_agent_of_another_host_is_named_with_its_host() {
    let file = with(
        other("one", &counts(1, 1, 1, 1)),
        json!({ "agents": [with(counts(1, 1, 1, 1), json!({ "agent": "main", "model": "m" }))] }),
    );
    assert_eq!(
        sums(&[file], &[])["agents"][0]["who"],
        "other-agent main agent"
    );
}

#[test]
fn each_submission_has_the_counts_of_the_requests_that_ended_while_it_was_open() {
    let sums = sums(&two_sessions(), &[]);
    assert_eq!(
        sums["submissions"],
        json!([{ "submission": "sub", "tokens": counts(10, 0, 0, 1) }])
    );
}

#[test]
fn with_no_totals_file_nothing_is_counted() {
    let sums = sums(&[], &[]);
    assert_eq!(sums["all"], json!({}));
    assert_eq!(sums["counted"], false);
    assert_eq!(sums["partial"], false);
}

#[test]
fn a_zero_cache_write_is_summed_as_its_totals_file_gives_it_whatever_the_host_or_format() {
    for host in ["codex", "claude-code"] {
        for format in [None, Some("yardtokens/1"), Some("yardtokens/2")] {
            let mut file = codex_totals();
            file["host"] = json!(host);
            if let Some(format) = format {
                file["format"] = json!(format);
            }
            let sums = sums(&[file], &[]);
            for counts in every_sum(&sums) {
                assert_eq!(counts["cacheWrite"], 0, "{host} {format:?}: {counts}");
            }
            assert_eq!(sums["partial"], false, "{host} {format:?}");
        }
    }
}

#[test]
fn a_totals_file_of_a_newer_format_is_left_out_of_the_sums_and_makes_them_partial() {
    let mut newer = other("two", &counts(5, 0, 0, 1));
    newer["format"] = json!("yardtokens/3");
    let sums = sums(&[other("one", &counts(2, 0, 0, 3)), newer], &[]);
    assert_eq!(sums["all"]["input"], 2);
    assert_eq!(sums["all"]["output"], 3);
    assert_eq!(sums["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(sums["sessions"][0]["session"], "one");
    assert_eq!(sums["partial"], true);
}

#[test]
fn token_accounting_completeness_is_a_property_of_the_whole_log() {
    let complete = counts(0, 0, 0, 0);
    let partial =
        |files: &[Value], activities: &[Value]| sums(files, activities)["partial"].clone();
    let session = |id: &str| json!({ "host": "other-agent", "session": id });
    assert_eq!(partial(&[other("one", &complete)], &[]), false);
    let marked = with(complete.clone(), json!({ "partial": ["input"] }));
    assert_eq!(partial(&[other("one", &marked)], &[]), true);
    let lacking = json!({ "input": 2, "output": 3 });
    assert_eq!(partial(&[other("one", &lacking)], &[]), true);
    let unreporting = [session("two")];
    assert_eq!(partial(&[other("one", &complete)], &unreporting), true);
    assert_eq!(partial(&[], &unreporting), true);
    assert_eq!(
        partial(&[other("one", &complete)], &[session("one")]),
        false
    );
}

#[test]
fn a_counter_a_contributing_totals_file_is_missing_makes_the_sum_partial() {
    let files = [
        other("one", &json!({ "input": 2, "output": 3 })),
        other("two", &json!({ "output": 4 })),
    ];
    let sums = sums(&files, &[]);
    assert_eq!(sums["all"]["input"], 2);
    assert_eq!(sums["all"]["output"], 7);
    assert_eq!(sums["all"]["partial"], json!(["input"]));
}

#[test]
fn a_counter_missing_from_every_contributing_totals_file_is_unavailable_in_the_sum() {
    let files = [
        other("one", &json!({ "input": 2, "output": 3 })),
        other("two", &json!({ "input": 1, "output": 4 })),
    ];
    let sums = sums(&files, &[]);
    assert_eq!(sums["all"], json!({ "input": 3, "output": 7 }));
    assert_eq!(sums["partial"], true);
}
