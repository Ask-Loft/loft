//! The session and turn portion of the local native protocol.

use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use super::Shared;

pub(super) fn reply(shared: &Shared, method: &str, params: &Value) -> Option<Value> {
    let mut threads = shared.threads.lock().unwrap();
    let state = threads.get_mut(params["threadId"].as_str()?)?;
    match method {
        "thread/read" => Some(
            json!({"thread":{"id":params["threadId"],"cwd":state.directory,"status":{"type":if state.active_turn.is_some(){"active"}else{"idle"}}}}),
        ),
        "thread/turns/list" => {
            let data: Vec<_> = state
                .active_turn
                .iter()
                .map(|id| json!({"id":id,"status":"inProgress","items":[],"itemsView":"notLoaded"}))
                .collect();
            Some(json!({"data":data,"nextCursor":null}))
        }
        "thread/items/list" => {
            let offset = params["cursor"]
                .as_str()
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(0);
            let count = params["limit"].as_u64().unwrap_or(32).try_into().ok()?;
            let data: Vec<_> = state
                .items
                .iter()
                .rev()
                .skip(offset)
                .take(count)
                .cloned()
                .collect();
            let next = (offset + data.len() < state.items.len())
                .then(|| (offset + data.len()).to_string());
            Some(json!({"data":data,"nextCursor":next}))
        }
        "turn/start" | "turn/steer" => {
            if method == "turn/steer"
                && (state.active_turn.is_none()
                    || state.active_turn.as_deref() != params["expectedTurnId"].as_str())
            {
                return None;
            }
            let turn = state
                .active_turn
                .get_or_insert_with(|| {
                    format!(
                        "plant-turn-{}",
                        shared.turns.fetch_add(1, Ordering::Relaxed)
                    )
                })
                .clone();
            state.items.push(json!({"turnId":turn,"item":{"type":"userMessage","id":format!("item-{}",state.items.len()),"clientId":params["clientUserMessageId"],"content":params["input"]}}));
            Some(if method == "turn/start" {
                json!({"turn":{"id":turn,"status":"inProgress"}})
            } else {
                json!({"turnId":turn})
            })
        }
        _ => None,
    }
}
