use serde_json::{Value, json};

use yardlog::totals::Totals;

use super::count;
use crate::yard::event::Event;
use crate::yard::session_files::as_read;

fn request(second: u32, more: &Value) -> Event {
    let mut posted = json!({"host":"codex","session":"s1","dir":"/work",
        "at":format!("2026-01-01T10:00:{second:02}Z"),"kind":"request","model":"fixture"});
    for (name, value) in more.as_object().unwrap() {
        posted[name] = value.clone();
    }
    Event::read(&posted).unwrap()
}

#[test]
fn a_totals_file_read_back_is_written_as_the_same_text() {
    let mut totals = Totals::new("s1");
    let measured = json!({"input":70,"cacheRead":30,"cacheWrite":0,"output":5});
    let partly = json!({"input":7,"cacheRead":3,"output":1});
    let sub = json!({"id":"r1","type":"review"});
    count(
        &mut totals,
        &request(1, &json!({"tokens":measured})),
        Some("shape"),
        &[],
    );
    let open = ["sub".to_owned()];
    let event = request(2, &json!({"tokens":partly,"agent":sub}));
    count(&mut totals, &event, Some("shape"), &open);
    count(
        &mut totals,
        &request(3, &json!({"tokens":measured})),
        None,
        &open,
    );
    count(&mut totals, &request(4, &json!({})), None, &[]);
    let compaction = request(5, &json!({"kind":"compaction","tokens":measured}));
    count(&mut totals, &compaction, Some("build"), &[]);
    let text = totals.text();
    assert!(text.contains("partial:"), "{text}");
    assert!(text.contains("type: review"), "{text}");
    assert!(text.contains("noStage:"), "{text}");
    assert!(text.contains("submissions:"), "{text}");
    let read = Totals::read(&as_read(&text));
    assert_eq!(read.text(), text);
}
