use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::json;
use software_plant::codex::{CodexAppServer, Scenario};

use super::{Connection, Notice, endpoint};

#[test]
fn socket_selection_uses_an_absolute_override_then_codex_home_then_home() {
    let root = Some(PathBuf::from("/codex"));
    let home = Some(PathBuf::from("/home/person"));
    assert_eq!(
        endpoint(None, root.clone(), home.clone()).unwrap().unwrap(),
        Path::new("/codex/app-server-control/app-server-control.sock")
    );
    assert_eq!(
        endpoint(None, None, home.clone()).unwrap().unwrap(),
        Path::new("/home/person/.codex/app-server-control/app-server-control.sock")
    );
    assert_eq!(
        endpoint(Some(Path::new("/custom.sock")), root.clone(), home.clone())
            .unwrap()
            .unwrap(),
        Path::new("/custom.sock")
    );
    assert!(endpoint(Some(Path::new("relative.sock")), root, home).is_err());
}

fn server() -> CodexAppServer {
    CodexAppServer::start(Scenario {
        loaded: vec!["native-thread".into()],
        ..Scenario::default()
    })
}

#[test]
fn a_loaded_thread_is_observed_without_configuration_or_turn_mutations() {
    let server = server();
    let connection = Connection::connect(server.path(), "native-thread").unwrap();
    drop(connection);
    std::thread::sleep(Duration::from_millis(60));
    let requests = server.requests();
    let methods: Vec<_> = requests
        .iter()
        .map(|r| r["method"].as_str().unwrap())
        .collect();
    assert_eq!(
        methods,
        [
            "initialize",
            "initialized",
            "thread/loaded/list",
            "thread/resume",
            "thread/unsubscribe"
        ]
    );
    assert_eq!(
        requests[3]["params"],
        json!({"threadId":"native-thread","excludeTurns":true})
    );
}

#[test]
fn an_unloaded_thread_is_never_resumed() {
    let server = CodexAppServer::start(Scenario::default());
    assert!(Connection::connect(server.path(), "native-thread").is_err());
    assert!(
        !server
            .requests()
            .iter()
            .any(|r| r["method"] == "thread/resume")
    );
}

#[test]
fn silent_and_refused_connections_are_bounded() {
    for scenario in [
        Scenario {
            silent: true,
            ..Scenario::default()
        },
        Scenario {
            loaded: vec!["native-thread".into()],
            refuse_subscription: true,
            ..Scenario::default()
        },
    ] {
        let server = CodexAppServer::start(scenario);
        let began = Instant::now();
        assert!(Connection::connect(server.path(), "native-thread").is_err());
        assert!(began.elapsed() < Duration::from_millis(450));
    }
    assert!(
        Connection::connect(
            Path::new("/tmp/loft-missing-app-server.sock"),
            "native-thread"
        )
        .is_err()
    );
}

#[test]
fn only_accounting_fields_are_retained_from_notifications() {
    let server = server();
    let mut connection = Connection::connect(server.path(), "native-thread").unwrap();
    server.emit(&json!({"method":"item/started","params":{"text":"PRIVATE PROMPT"}}));
    server.emit(&json!({"method":"thread/tokenUsage/updated","params":{"threadId":"native-thread","turnId":"turn","tokenUsage":{"last":{"inputTokens":100,"outputTokens":5,"private":"PRIVATE"},"total":{"inputTokens":100},"private":"PRIVATE"}}}));
    let began = Instant::now();
    let notices = loop {
        let notices = connection.poll().unwrap();
        if !notices.is_empty() {
            break notices;
        }
        assert!(began.elapsed() < Duration::from_secs(1));
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(
        notices,
        vec![Notice::Usage {
            thread: "native-thread".into(),
            turn: "turn".into(),
            usage: json!({"last":{"inputTokens":100,"outputTokens":5},"total":{"inputTokens":100}})
        }]
    );
}

#[test]
fn malformed_oversized_and_disconnected_peers_are_unavailable_without_accounting() {
    for raw in [
        Some("{malformed".to_owned()),
        Some("x".repeat(1_048_577)),
        None,
    ] {
        let server = server();
        let mut connection = Connection::connect(server.path(), "native-thread").unwrap();
        if let Some(raw) = raw {
            server.raw(raw);
        } else {
            server.disconnect();
        }
        let began = Instant::now();
        loop {
            match connection.poll() {
                Err(_) => break,
                Ok(notices) => assert_eq!(notices, []),
            }
            assert!(began.elapsed() < Duration::from_secs(1));
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[test]
fn scripted_snapshots_preserve_duplicates_resets_and_absent_optional_counters() {
    let sample = |input| json!({"method":"thread/tokenUsage/updated","params":{"threadId":"native-thread","turnId":"turn","tokenUsage":{"last":{"inputTokens":input,"cachedInputTokens":0,"outputTokens":0},"total":{"inputTokens":input,"cachedInputTokens":0,"outputTokens":0}}}});
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["native-thread".into()],
        on_subscribe: vec![
            sample(100),
            sample(100),
            sample(0),
            sample(10),
            json!({"method":"item/started","params":{"text":"PRIVATE"}}),
        ],
        ..Scenario::default()
    });
    let mut connection = Connection::connect(peer.path(), "native-thread").unwrap();
    let began = Instant::now();
    let mut notices = vec![];
    while notices.len() < 4 {
        notices.extend(connection.poll().unwrap());
        assert!(began.elapsed() < Duration::from_secs(1));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(notices[0], notices[1]);
    for (notice, input) in notices.iter().zip([100, 100, 0, 10]) {
        let Notice::Usage { usage, .. } = notice else {
            panic!("unexpected completion")
        };
        assert_eq!(usage["total"]["inputTokens"], input);
        assert!(usage["last"].get("cacheWriteInputTokens").is_none());
    }
}
