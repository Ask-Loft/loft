use std::path::Path;

use serde_json::json;
use software_plant::codex::{CodexAppServer, NativeThread, Scenario};

use super::{CallFailure, Client};

#[test]
fn an_idle_loaded_thread_can_be_inspected_without_returning_its_conversation() {
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["thread".into()],
        ..Scenario::default()
    });
    let mut client = Client::connect(peer.path(), "thread").unwrap();
    assert_eq!(client.inspect(Path::new("/work")).unwrap(), None);
    let requests = peer.requests();
    let subscription = requests
        .iter()
        .find(|r| r["method"] == "thread/resume")
        .unwrap();
    assert_eq!(
        subscription["params"],
        json!({"threadId":"thread","excludeTurns":true})
    );
}

#[test]
fn idle_delivery_is_receipted_and_a_repeated_start_joins_the_active_turn() {
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["thread".into()],
        ..Scenario::default()
    });
    let mut client = Client::connect(peer.path(), "thread").unwrap();
    assert!(!client.received("yard-marker").unwrap());
    let turn = client
        .deliver("yard-marker", "yard-marker\nRead Yard.", None)
        .unwrap();
    assert!(client.received("yard-marker").unwrap());
    assert_eq!(
        client.inspect(Path::new("/work")).unwrap(),
        Some(turn.clone())
    );
    assert_eq!(client.deliver("other", "other", None).unwrap(), turn);
    assert_eq!(
        client.deliver("stale", "stale", Some("old")),
        Err(CallFailure::Refused)
    );
    assert_eq!(client.deliver("steer", "steer", Some(&turn)).unwrap(), turn);
    for request in peer
        .requests()
        .iter()
        .filter(|r| r["method"] == "turn/start" || r["method"] == "turn/steer")
    {
        let params = request["params"].as_object().unwrap();
        assert!(params.keys().all(|key| {
            ["threadId", "clientUserMessageId", "input", "expectedTurnId"].contains(&key.as_str())
        }));
    }
}

#[test]
fn an_unloaded_thread_or_different_directory_is_not_used_for_delivery() {
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["thread".into()],
        ..Scenario::default()
    });
    assert!(Client::connect(peer.path(), "other").is_err());
    let mut client = Client::connect(peer.path(), "thread").unwrap();
    assert_eq!(
        client.inspect(Path::new("/unrelated")),
        Err(CallFailure::Refused)
    );
}

#[test]
fn a_notification_client_does_not_displace_an_existing_usage_client() {
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["thread".into()],
        refuse_methods: vec!["turn/start".into()],
        ..Scenario::default()
    });
    let mut usage = super::Connection::connect(peer.path(), "thread").unwrap();
    let mut client = Client::connect(peer.path(), "thread").unwrap();
    assert_eq!(client.inspect(Path::new("/work")).unwrap(), None);
    assert_eq!(
        client.deliver("refused", "refused", None),
        Err(CallFailure::Refused)
    );
    peer.emit(&json!({"method":"thread/tokenUsage/updated","params":{"threadId":"thread","turnId":"turn","tokenUsage":{"last":{"inputTokens":1},"total":{"inputTokens":1}}}}));
    // Two request boundaries let the real socket peer flush its queued notice without a sleep.
    for _ in 0..2 {
        usage.request("thread/loaded/list", &json!({})).unwrap();
    }
    assert!(
        usage
            .poll()
            .unwrap()
            .iter()
            .any(|notice| matches!(notice, super::super::Notice::Usage { .. }))
    );
}

#[test]
fn a_lost_acknowledgment_can_be_recovered_from_the_native_receipt_after_reconnect() {
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["thread".into()],
        drop_acknowledgements: vec!["turn/start".into()],
        ..Scenario::default()
    });
    let mut client = Client::connect(peer.path(), "thread").unwrap();
    assert_eq!(
        client.deliver("yard-lost", "yard-lost\nRead Yard.", None),
        Err(CallFailure::Unavailable)
    );
    drop(client);
    let mut recovered = Client::connect(peer.path(), "thread").unwrap();
    assert!(recovered.received("yard-lost").unwrap());
    assert_eq!(
        peer.requests()
            .iter()
            .filter(|r| r["method"] == "turn/start")
            .count(),
        1
    );
}

#[test]
fn receipt_inspection_stops_after_its_bounded_pages_and_ignores_assistant_text() {
    let mut items =
        vec![json!({"item":{"type":"userMessage","content":[{"type":"text","text":"yard-old"}]}})];
    items.extend((0..140).map(|n| json!({"item":{"type":"agentMessage","id":n.to_string(),"text":"yard-assistant","clientId":"yard-assistant"}})));
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["thread".into()],
        threads: [(
            "thread".into(),
            NativeThread {
                items,
                ..NativeThread::default()
            },
        )]
        .into(),
        ..Scenario::default()
    });
    let mut client = Client::connect(peer.path(), "thread").unwrap();
    assert!(!client.received("yard-old").unwrap());
    assert_eq!(
        peer.requests()
            .iter()
            .filter(|r| r["method"] == "thread/items/list")
            .count(),
        4
    );
    assert!(!client.received("yard-assistant").unwrap());
}

#[test]
fn an_explicit_native_refusal_is_distinct_from_an_uncertain_transport_failure() {
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["thread".into()],
        refuse_methods: vec!["turn/start".into()],
        ..Scenario::default()
    });
    let mut client = Client::connect(peer.path(), "thread").unwrap();
    assert_eq!(
        client.deliver("yard-refused", "yard-refused", None),
        Err(CallFailure::Refused)
    );
    assert!(!client.received("yard-refused").unwrap());
}
