//! Decision delivery shares the bounded local transport, independently of usage observation.

use std::path::Path;

use serde_json::{Value, json};

use super::{CallFailure, Connection};

type Result<T> = std::result::Result<T, CallFailure>;

pub struct Client {
    connection: Connection,
    thread: String,
}

impl Client {
    pub fn connect(path: &Path, thread: &str) -> Result<Self> {
        Ok(Self {
            connection: Connection::connect(path, thread).map_err(|_| CallFailure::Unavailable)?,
            thread: thread.into(),
        })
    }

    /// Only runtime metadata and a turn identifier escape the protocol boundary.
    pub fn inspect(&mut self, directory: &Path) -> Result<Option<String>> {
        let read = self.request("thread/read", &json!({"threadId":self.thread}))?;
        let thread = &read["thread"];
        let cwd = thread["cwd"].as_str().ok_or(CallFailure::Unavailable)?;
        if !same_directory(Path::new(cwd), directory) {
            return Err(CallFailure::Refused);
        }
        match thread["status"]["type"].as_str() {
            Some("idle") => Ok(None),
            Some("active") => {
                let turns = self.request("thread/turns/list", &json!({"threadId":self.thread,"limit":1,"sortDirection":"desc","itemsView":"notLoaded"}))?;
                turns["data"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|turn| turn["status"] == "inProgress")
                    .and_then(|turn| turn["id"].as_str())
                    .map(|id| Some(id.to_owned()))
                    .ok_or(CallFailure::Unavailable)
            }
            _ => Err(CallFailure::Unavailable),
        }
    }

    /// Receipt inspection is bounded to 128 structured items and keeps no conversation text.
    pub fn received(&mut self, marker: &str) -> Result<bool> {
        let mut cursor = Value::Null;
        for _ in 0..4 {
            let page = self.request(
                "thread/items/list",
                &json!({"threadId":self.thread,"limit":32,"sortDirection":"desc","cursor":cursor}),
            )?;
            let items = page["data"].as_array().ok_or(CallFailure::Unavailable)?;
            if items.iter().any(|entry| is_receipt(&entry["item"], marker)) {
                return Ok(true);
            }
            cursor = page["nextCursor"].clone();
            if cursor.is_null() {
                return Ok(false);
            }
            if !cursor.is_string() {
                return Err(CallFailure::Unavailable);
            }
        }
        Ok(false)
    }

    pub fn deliver(&mut self, marker: &str, text: &str, active: Option<&str>) -> Result<String> {
        let mut params = json!({"threadId":self.thread,"clientUserMessageId":marker,"input":[{"type":"text","text":text}]});
        let (method, result) = if let Some(turn) = active {
            params["expectedTurnId"] = json!(turn);
            ("turn/steer", "turnId")
        } else {
            ("turn/start", "turn")
        };
        let reply = self.request(method, &params)?;
        let id = if result == "turn" {
            reply[result]["id"].as_str()
        } else {
            reply[result].as_str()
        };
        id.map(str::to_owned).ok_or(CallFailure::Unavailable)
    }

    fn request(&mut self, method: &str, params: &Value) -> Result<Value> {
        self.connection.request(method, params)
    }
}

fn same_directory(a: &Path, b: &Path) -> bool {
    a == b
        || a.canonicalize()
            .ok()
            .zip(b.canonicalize().ok())
            .is_some_and(|(a, b)| a == b)
}

fn is_receipt(item: &Value, marker: &str) -> bool {
    item["type"] == "userMessage"
        && (item["clientId"] == marker
            || item["content"].as_array().is_some_and(|content| {
                content.iter().any(|part| {
                    part["type"] == "text"
                        && part["text"]
                            .as_str()
                            .is_some_and(|text| text.starts_with(marker))
                })
            }))
}

#[cfg(all(test, unix))]
#[path = "notification_connection_tests.rs"]
mod tests;

#[cfg(all(test, unix))]
#[path = "notification_contract.rs"]
mod contract;
