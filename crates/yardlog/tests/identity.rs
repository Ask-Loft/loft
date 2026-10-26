//! The same actor contract in memory and after canonical files are read.

use std::collections::BTreeMap;

use serde_json::json;
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
