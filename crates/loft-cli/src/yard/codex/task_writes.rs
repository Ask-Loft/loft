//! Shared optimistic task writes and exact assignment lookup for native hooks.

use serde_json::{Value, json};

use super::Result;
use super::commands::{self, Caller};
use super::translate::Hook;

pub fn actor_key(caller: &Caller) -> String {
    json!([caller.session, caller.agent.as_ref().map(|(id, _)| id)]).to_string()
}

pub fn tasks(doc: &Value) -> impl Iterator<Item = (&Value, &Value)> {
    doc["stages"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|stage| {
            stage["blocks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|block| block["kind"] == "task")
                .map(move |block| (stage, block))
        })
}

pub fn assigned<'a>(
    doc: &'a Value,
    caller: &'a Caller,
) -> impl Iterator<Item = (&'a Value, &'a Value)> {
    tasks(doc).filter(|(_, block)| caller.owns(&block["version"]["body"]["assignee"]))
}

pub fn executable(body: &Value, caller: &Caller) -> bool {
    !body["assignee"].is_object() || caller.owns(&body["assignee"])
}

pub fn changed(stage: &Value, held: &Value, status: &str) -> Option<Value> {
    let body = &held["version"]["body"];
    if body["state"] == status {
        return None;
    }
    let mut next = body.clone();
    next["state"] = json!(status);
    next["id"] = held["id"].clone();
    next["kind"] = json!("task");
    next["stage"] = stage["id"].clone();
    Some(next)
}

pub fn update(
    hook: &Hook,
    caller: &Caller,
    port: u16,
    select: impl Fn(&Value) -> Vec<Value>,
) -> Result<()> {
    retry(
        || commands::read(hook.dir(), caller, port),
        |doc, batch| {
            let mut args = vec![
                "write".into(),
                "--input".into(),
                "-".into(),
                "--based-on".into(),
                doc["revision"].to_string(),
            ];
            args.extend(caller.args());
            commands::run(hook.dir(), &args, port, Some(&json!(batch)))
        },
        select,
    )
}

fn retry(
    mut read: impl FnMut() -> Result<Value>,
    mut write: impl FnMut(&Value, Vec<Value>) -> Result<Value>,
    select: impl Fn(&Value) -> Vec<Value>,
) -> Result<()> {
    for _ in 0..3 {
        let doc = read()?;
        let batch = select(&doc);
        if batch.is_empty() {
            return Ok(());
        }
        let written = write(&doc, batch)?;
        if written["outcome"] == "written" {
            return Ok(());
        }
        if !written["errors"]
            .as_array()
            .is_some_and(|errors| errors.iter().any(|error| error["kind"] == "stale"))
        {
            return Err(format!("Yard refused a task mirror: {}", written["errors"]));
        }
    }
    Err("Yard changed during three task mirror attempts".to_owned())
}

pub fn finish(hook: &Hook, caller: &Caller, port: u16) -> Result<()> {
    update(hook, caller, port, |doc| {
        assigned(doc, caller)
            .filter(|(_, block)| block["version"]["body"]["state"] == "active")
            .filter_map(|(stage, block)| changed(stage, block, "open"))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::{assigned, changed, retry};
    use crate::yard::codex::commands::Caller;
    use serde_json::json;
    use std::cell::RefCell;

    #[test]
    fn stale_task_retries_reselect_the_current_assignment_and_preserve_unrelated_fields() {
        scenario(false);
        scenario(true);
    }

    fn scenario(reassign: bool) {
        let caller = Caller {
            session: "s".into(),
            agent: Some(("worker".into(), "builder".into())),
        };
        let doc = RefCell::new(
            json!({"revision":1,"stages":[{"id":"build","blocks":[{"id":"shared","kind":"task","version":{"body":{"title":"Keep","detail":"Before","state":"active","assignee":{"host":"codex","session":"s","agent":"worker"}}}}]}]}),
        );
        let writes = RefCell::new(vec![]);
        retry(
            || Ok(doc.borrow().clone()),
            |read, batch| {
                writes.borrow_mut().push(batch.clone());
                if read["revision"] != 1 {
                    return Ok(json!({"outcome":"written"}));
                }
                let mut latest = doc.borrow_mut();
                latest["revision"] = json!(2);
                latest["stages"][0]["blocks"][0]["version"]["body"]["detail"] =
                    json!("Concurrent detail");
                if reassign {
                    latest["stages"][0]["blocks"][0]["version"]["body"]["assignee"]["agent"] =
                        json!("other");
                }
                Ok(json!({"outcome":"refused","errors":[{"kind":"stale"}]}))
            },
            |doc| {
                assigned(doc, &caller)
                    .filter_map(|(stage, block)| changed(stage, block, "done"))
                    .collect()
            },
        )
        .unwrap();
        let writes = writes.into_inner();
        assert_eq!(writes.len(), if reassign { 1 } else { 2 });
        if !reassign {
            assert_eq!(writes[1][0]["detail"], "Concurrent detail");
        }
    }
}
