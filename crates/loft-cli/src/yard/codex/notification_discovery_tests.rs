use serde_json::json;

use super::*;
use crate::yard::codex::commands::Caller;

fn route() -> Route {
    Route {
        caller: Caller {
            session: "session".into(),
            agent: None,
        },
        thread: "session".into(),
        directory: "/work/subdir".into(),
    }
}

fn history() -> Value {
    json!({"schema":"loft-yard-read/3","outcome":"read","revision":8,
    "tree":"/work","branch":"feat/shelf","yardLog":{"key":"log","closedBy":"open"},
    "lastRead":8,"ownLastRead":null,"sent":[
        {"key":"pending","state":"open","reached":1,"agent":{"host":"codex","session":"session"}},
        {"key":"resolved","state":"answered","reached":5,"agent":{"host":"codex","session":"session"}},
        {"key":"withdrawn","state":"withdrawn","reached":6,"agent":{"host":"codex","session":"session"}}
    ]})
}

#[test]
fn discovery_offers_only_fully_resolved_submissions_without_borrowing_another_readers_progress() {
    let snapshot = Snapshot::read(&history(), &route()).unwrap();
    assert_eq!(snapshot.resolved.len(), 1);
    assert_eq!(snapshot.resolved[0].submission, "resolved");
    assert_eq!(snapshot.resolved[0].revision, 5);
    assert_eq!(snapshot.own_read, None);
    assert_eq!(snapshot.withdrawn, ["yard-decision-log-withdrawn"]);
    let mut read = history();
    read["ownLastRead"] = json!(7);
    assert_eq!(Snapshot::read(&read, &route()).unwrap().own_read, Some(7));
}

#[test]
fn discovery_requires_the_exact_host_session_and_subagent_sender() {
    let mut doc = history();
    let owner = &mut doc["sent"][1]["agent"];
    owner["agent"] = json!("reviewer");
    assert_eq!(Snapshot::read(&doc, &route()).unwrap().resolved, vec![]);
    let mut sub = route();
    sub.caller.agent = Some(("reviewer".into(), "review".into()));
    sub.thread = "reviewer".into();
    assert_eq!(
        Snapshot::read(&doc, &sub).unwrap().resolved[0].thread,
        "reviewer"
    );
    for (field, value) in [("host", "claude"), ("session", "other"), ("agent", "other")] {
        let mut foreign = doc.clone();
        foreign["sent"][1]["agent"][field] = json!(value);
        assert_eq!(Snapshot::read(&foreign, &sub).unwrap().resolved, vec![]);
    }
}

#[test]
fn a_fresh_source_check_refuses_closed_withdrawn_or_different_logs_trees_and_branches() {
    let snapshot = Snapshot::read(&history(), &route()).unwrap();
    let resolution = &snapshot.resolved[0];
    assert!(snapshot.contains(resolution));
    for (field, value) in [("branch", "other"), ("tree", "/other")] {
        let mut changed = history();
        changed[field] = json!(value);
        assert!(!Snapshot::read(&changed, &route()).is_some_and(|s| s.contains(resolution)));
    }
    let mut changed = history();
    changed["yardLog"]["key"] = json!("other-log");
    assert!(
        !Snapshot::read(&changed, &route())
            .unwrap()
            .contains(resolution)
    );
    changed = history();
    changed["yardLog"]["closedBy"] = json!("complete");
    assert!(
        !Snapshot::read(&changed, &route())
            .unwrap()
            .contains(resolution)
    );
    changed = history();
    changed["sent"][1]["state"] = json!("withdrawn");
    assert!(
        !Snapshot::read(&changed, &route())
            .unwrap()
            .contains(resolution)
    );
}

#[test]
fn incomplete_or_unsupported_history_is_not_authoritative() {
    for (field, value) in [
        ("schema", json!("loft-yard-read/2")),
        ("outcome", json!("error")),
        ("leftOut", json!(1)),
        ("sent", Value::Null),
    ] {
        let mut doc = history();
        doc[field] = value;
        assert!(Snapshot::read(&doc, &route()).is_none());
    }
    let mut doc = history();
    doc["sent"][1].as_object_mut().unwrap().remove("reached");
    assert_eq!(Snapshot::read(&doc, &route()).unwrap().resolved, vec![]);
}
