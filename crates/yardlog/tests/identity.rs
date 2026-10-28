//! The same actor contract in memory and after canonical files are read.

use std::collections::BTreeMap;

use serde_json::json;
use yardlog::identity::SessionKey;
use yardlog::{Agent, YardLog};

use crate::common::{context, opened, write};

#[test]
fn identical_session_and_subagent_ids_in_two_hosts_keep_their_own_active_stages() {
    let mut log = opened();
    let mut agents = Vec::new();
    for host in ["claude-code", "other-agent"] {
        agents.push(Agent::main_in(host, "same"));
        agents.push(Agent::sub_in(host, "same", "sub", "reader"));
    }
    for (n, agent) in agents.iter().enumerate() {
        write(
            &mut log,
            &json!([{
                "id": format!("stage-{n}"), "kind": "stage", "title": "Work", "status": "active"
            }]),
            &context(agent, 1),
        );
    }
    let files: BTreeMap<String, String> = log.files().into_iter().collect();
    let read = YardLog::read(files[yardlog::codec::ROOT].as_bytes(), |name| {
        files.get(name).map(|text| text.as_bytes().to_vec())
    })
    .unwrap();
    assert_eq!(read, log);
    for held in [&log, &read] {
        for (n, agent) in agents.iter().enumerate() {
            let stages: Vec<_> = held.active_stages_of(agent).collect();
            assert_eq!(stages.len(), 1);
            assert_eq!(stages[0].id.as_str(), format!("stage-{n}"));
        }
    }
    assert_eq!(Agent::main("same"), Agent::main_in("claude-code", "same"));
    assert_eq!(Agent::sub("same", "sub", "reader"), agents[1]);
}

#[test]
fn a_session_key_reads_back_as_the_host_and_session_it_was_made_from() {
    for (host, session, stored) in [
        ("claude-code", "abc-1", "abc-1"),
        ("codex", "019a-77", "codex~019a-77"),
        ("other-agent", "same", "other-agent~same"),
    ] {
        let key = SessionKey::new(host, session);
        assert_eq!(key.to_string(), stored);
        assert_eq!(yardlog::identity::session_key(host, session), stored);
        let read = SessionKey::parse(stored);
        assert_eq!(read, key);
        assert_eq!((read.host(), read.session()), (host, session));
        assert!(read.is_valid());
    }
    assert_ne!(
        SessionKey::new("claude-code", "same").to_string(),
        SessionKey::new("codex", "same").to_string()
    );
}

#[test]
fn a_session_key_with_an_unusable_host_or_session_cannot_name_a_file() {
    for stored in [
        "",
        "Codex~abc",
        "codex~",
        "codex~.hidden",
        "codex~a/b",
        "../x",
    ] {
        assert!(!SessionKey::parse(stored).is_valid(), "{stored}");
    }
    assert_eq!(SessionKey::parse("codex~a~b").session(), "a~b");
}
