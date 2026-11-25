use serde_json::json;

use super::{Event, arrivals};

fn event(session: &str, dir: &str) -> Event {
    let posted = json!({
        "session": session, "dir": dir, "at": "2026-10-22T09:30:00Z", "kind": "turn.start"
    });
    Event::read(&posted).unwrap()
}

#[test]
fn a_batch_finds_each_agent_s_working_tree_once_and_opens_the_yard_log_there() {
    let events: Vec<Event> = (0..100)
        .map(|n| event(if n % 2 == 0 { "s1" } else { "s2" }, "/work/a"))
        .collect();
    let mut asked = Vec::new();
    let mut opened = Vec::new();
    arrivals(
        &events,
        |event| {
            asked.push(event.session.clone());
            if event.session == "s1" {
                "/work/b"
            } else {
                "/work/a"
            }
            .to_owned()
        },
        |dir| {
            opened.push(dir.to_owned());
            None
        },
    );
    asked.sort();
    assert_eq!(asked, ["s1", "s2"]);
    opened.sort();
    assert_eq!(opened, ["/work/a", "/work/b"]);
}

#[test]
fn a_batch_opens_the_yard_log_of_each_directory_it_names_once() {
    let events: Vec<Event> = (0..1000)
        .map(|n| {
            let dir = if n % 2 == 0 { "/work/a" } else { "/work/b" };
            let posted = json!({
                "session": "s1", "dir": dir, "at": "2026-10-22T09:30:00Z", "kind": "turn.start"
            });
            Event::read(&posted).unwrap()
        })
        .collect();
    let mut opened = Vec::new();
    let found = arrivals(
        &events,
        |event| event.dir.clone(),
        |dir| {
            opened.push(dir.to_owned());
            None
        },
    );
    assert_eq!(found.len(), 1000);
    opened.sort();
    assert_eq!(opened, ["/work/a", "/work/b"]);
}
