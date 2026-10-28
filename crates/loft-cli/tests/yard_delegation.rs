//! Assigned task identities and derived execution attempts through the unchanged JSON commands.

use crate::yard::{Yard, block};

#[test]
fn assigned_workers_update_the_same_task_and_reads_expose_closed_execution_attempts() {
    let yard = Yard::new();
    yard.write(
        "s1",
        r#"[
        {"id":"build","kind":"stage","title":"Build","status":"active"},
        {"id":"work","kind":"task","stage":"build","title":"Work","state":"open",
         "assignee":{"host":"claude-code","session":"s1","agent":"worker","agentType":"builder"}}
    ]"#,
        &[],
    );
    let original = yard.read("s1", &[]);
    for state in ["active", "open", "active", "done"] {
        let batch = format!(
            r#"[{{"id":"work","kind":"task","stage":"build","title":"Work","state":"{state}",
            "assignee":{{"host":"claude-code","session":"s1","agent":"worker","agentType":"builder"}}}}]"#
        );
        yard.write(
            "s1",
            &batch,
            &["--agent", "worker", "--agent-type", "builder"],
        );
    }
    let read = yard.read("s1", &[]);
    let task = block(&read, "work");
    assert_eq!(task["key"], block(&original, "work")["key"]);
    assert_eq!(task["stage"], original["stages"][0]["key"]);
    assert_eq!(task["version"]["body"]["assignee"]["agent"], "worker");
    assert_eq!(task["executions"].as_array().unwrap().len(), 2);
    assert!(
        task["executions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|span| span["ended"].is_string())
    );
    assert_eq!(read["stages"][0]["owner"]["session"], "s1");
}
