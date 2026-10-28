//! A session's totals file names its totals format: every file is written in `yardtokens/2`, and
//! a file that names no format, such as one that carries only an earlier build's marker, reads as
//! `yardtokens/1`.

use serde_json::json;
use yardlog::totals::{Totals, TotalsFormat};

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
