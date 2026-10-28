//! A session's totals file names its totals format: every file is written in `yardtokens/2`, and
//! a file that names no format, such as one that carries only an earlier build's marker, reads as
//! `yardtokens/1`. The counts of totals map in every scope they hold, such as an upgrade making
//! each zero cache-write count unavailable.

use serde_json::json;
use yardlog::totals::{Totals, TotalsFormat};
use yardlog::usage::Counts;

#[test]
fn every_totals_file_is_written_naming_the_current_totals_format() {
    let text = Totals::new("s1").text();
    assert!(text.starts_with("format: yardtokens/2\n"), "{text}");
    assert!(!text.contains("cacheWriteClassified"), "{text}");
    assert_eq!(TotalsFormat::CURRENT.to_string(), "yardtokens/2");
}

#[test]
fn a_totals_file_that_names_no_format_reads_as_the_first_totals_format() {
    let first = Some(TotalsFormat::new(1));
    assert_eq!(TotalsFormat::of(&json!({ "session": "s1" })), first);
    let marked = json!({ "session": "s1", "cacheWriteClassified": true });
    assert_eq!(TotalsFormat::of(&marked), first);
    let current = json!({ "format": "yardtokens/2" });
    assert_eq!(TotalsFormat::of(&current), Some(TotalsFormat::CURRENT));
    let newer = TotalsFormat::of(&json!({ "format": "yardtokens/3" })).unwrap();
    assert!(!newer.is_known());
    assert_eq!(TotalsFormat::of(&json!({ "format": "yardlog/1" })), None);
}

#[test]
fn mapping_the_counts_of_totals_reaches_every_scope_they_hold() {
    let zero = json!({"input":70,"cacheRead":30,"cacheWrite":0,"output":5});
    let file = json!({
        "format": "yardtokens/2", "session": "s1", "host": "codex",
        "tokens": {"input":140,"cacheRead":60,"cacheWrite":4,"output":10},
        "stages": [{"stage":"shape","input":70,"cacheRead":30,"cacheWrite":0,"output":5,
            "agents":[{"agent":"main","input":70,"cacheRead":30,"cacheWrite":0,"output":5}]}],
        "noStage": zero,
        "agents": [{"agent":"main","model":"fixture","input":70,"cacheRead":30,"cacheWrite":0,"output":5}],
        "submissions": [{"submission":"sub","input":70,"cacheRead":30,"cacheWrite":0,"output":5}],
    });
    let mut totals = Totals::read(&file);
    totals.map_counts(Counts::without_zero_write);
    let text = totals.text();
    assert!(!text.contains("cacheWrite: 0"), "{text}");
    assert!(text.contains("cacheWrite: 4"), "{text}");
    assert_eq!(text.matches("input: 70").count(), 5, "{text}");
}

#[test]
fn a_zero_cache_write_count_is_made_unavailable_and_every_other_count_kept() {
    let zero = Counts::read(&json!({"input":1,"cacheRead":0,"cacheWrite":0,"output":0}));
    let unavailable = Counts::read(&json!({"input":1,"cacheRead":0,"output":0}));
    assert_eq!(zero.without_zero_write(), unavailable);
    let positive = Counts::read(&json!({"input":1,"cacheRead":0,"cacheWrite":3,"output":0}));
    assert_eq!(positive.without_zero_write(), positive);
    assert_eq!(unavailable.without_zero_write(), unavailable);
}
