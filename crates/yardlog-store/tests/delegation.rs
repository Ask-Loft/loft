//! The same delegation and refusal sequence in memory and through the journaled file store.

use serde_json::json;
use yardlog::input::read_batch;
use yardlog::write::{Context, Reports};
use yardlog::{Agent, YardLog};

use crate::common::{Failed, Tree, agent, at, read};

#[test]
fn delegated_tasks_keep_identity_and_reassignment_guards_in_memory_and_files() {
    scenario(false);
    scenario(true);
}

fn scenario(persisted: bool) {
    let tree = Tree::new();
    let store = tree.store("feature");
    let mut memory = YardLog::open("feature", "develop", at(0), agent());
    let mut key = None;
    for (seconds, actor, task_state, assignee, accepted) in [
        (1, agent(), "open", "first", true),
        (
            2,
            Agent::sub("session-a", "first", "worker"),
            "active",
            "first",
            true,
        ),
        (3, agent(), "active", "second", true),
        (
            4,
            Agent::sub("session-a", "first", "worker"),
            "done",
            "second",
            false,
        ),
        (
            5,
            Agent::sub("session-a", "second", "worker"),
            "done",
            "second",
            true,
        ),
    ] {
        let mut batch = vec![];
        if seconds == 1 {
            batch.push(json!({"id":"build","kind":"stage","title":"Build","status":"active"}));
        }
        batch.push(json!({"id":"work","kind":"task","stage":"build","title":"Work","state":task_state,
                "assignee":{"host":"claude-code","session":"session-a","agent":assignee,"agentType":"worker"}}));
        let inputs = read_batch(&json!(batch)).unwrap();
        let context = Context {
            at: at(seconds),
            agent: actor,
            base: None,
            send: false,
        };
        if persisted {
            let before = tree.scratch.snapshot();
            let result = store.update(|current| -> Result<_, Failed> {
                let mut log = current.unwrap_or_else(|| memory.clone());
                let written = log
                    .write(&inputs, &context, &Reports::new())
                    .map_err(Failed::Refused)?;
                Ok((log, written))
            });
            assert_eq!(result.is_ok(), accepted);
            if !accepted {
                assert_eq!(tree.scratch.snapshot(), before);
            }
            memory = read(&store);
        } else {
            let before = memory.clone();
            assert_eq!(
                memory.write(&inputs, &context, &Reports::new()).is_ok(),
                accepted
            );
            if !accepted {
                assert_eq!(memory, before);
            }
        }
        let task = &memory.stages[0].blocks[0];
        assert_eq!(*key.get_or_insert(task.key), task.key);
        assert_eq!(memory.stages[0].stage.owner(), Some(&agent()));
    }
    assert_eq!(memory.stages[0].blocks[0].versions.len(), 4);
    assert_eq!(memory.stages[0].blocks[0].executions().len(), 2);
}
