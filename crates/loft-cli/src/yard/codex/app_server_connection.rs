//! Bounded WebSocket protocol over the local platform transport.

use std::collections::VecDeque;
use std::io::ErrorKind;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tungstenite::client::client_with_config;
use tungstenite::handshake::HandshakeError;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Error, Message, WebSocket};

use super::{CallFailure, Notice, Result};

#[path = "app_server_socket.rs"]
mod socket;

const FAILURE: &str = "local Codex app-server observation is unavailable";
const LIMIT: usize = 1_048_576;

pub struct Connection {
    socket: WebSocket<socket::Stream>,
    thread: String,
    next: u64,
    pending: VecDeque<Notice>,
    subscribed: bool,
    failed: bool,
}

impl Connection {
    pub fn connect(path: &Path, thread: &str) -> Result<Self> {
        let deadline = Instant::now() + Duration::from_millis(300);
        let stream = socket::connect(path).map_err(failed)?;
        let config = WebSocketConfig::default()
            .read_buffer_size(4096)
            .write_buffer_size(0)
            .max_write_buffer_size(LIMIT)
            .max_message_size(Some(LIMIT))
            .max_frame_size(Some(LIMIT));
        let mut handshake = client_with_config("ws://localhost/", stream, Some(config));
        let socket = loop {
            match handshake {
                Ok((socket, _)) => break socket,
                Err(HandshakeError::Interrupted(part)) => {
                    pause(deadline)?;
                    handshake = part.handshake();
                }
                Err(HandshakeError::Failure(error)) => return Err(failed(error)),
            }
        };
        let mut connection = Self {
            socket,
            thread: thread.into(),
            next: 0,
            pending: VecDeque::new(),
            subscribed: false,
            failed: false,
        };
        connection.call("initialize", &json!({"clientInfo":{"name":"loft-yard","version":crate::version::VERSION},"capabilities":{"experimentalApi":true}}), deadline)?;
        connection.send(&json!({"method":"initialized"}))?;
        let loaded = connection.call("thread/loaded/list", &json!({}), deadline)?;
        if !loaded["data"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(thread)))
        {
            return Err(FAILURE.into());
        }
        connection.call(
            "thread/resume",
            &json!({"threadId":thread,"excludeTurns":true}),
            deadline,
        )?;
        connection.subscribed = true;
        Ok(connection)
    }

    pub fn poll(&mut self) -> Result<Vec<Notice>> {
        if self.failed {
            return Err(FAILURE.into());
        }
        let mut result: Vec<_> = self.pending.drain(..).collect();
        // Bound work even when a native peer continuously produces unrelated notifications.
        for _ in 0..128 {
            let value = match self.read() {
                Ok(Some(value)) => value,
                Ok(None) => break,
                Err(error) if result.is_empty() => return Err(error),
                Err(_) => {
                    self.failed = true;
                    break;
                }
            };
            if let Some(notice) = notice(&value) {
                result.push(notice);
            }
        }
        Ok(result)
    }

    fn send(&mut self, value: &Value) -> Result<()> {
        self.socket
            .send(Message::Text(value.to_string().into()))
            .map_err(failed)
    }

    fn call(&mut self, method: &str, params: &Value, deadline: Instant) -> Result<Value> {
        self.request_until(method, params, deadline)
            .map_err(|_| FAILURE.to_owned())
    }

    pub(super) fn request(
        &mut self,
        method: &str,
        params: &Value,
    ) -> std::result::Result<Value, CallFailure> {
        self.request_until(method, params, Instant::now() + Duration::from_millis(300))
    }

    fn request_until(
        &mut self,
        method: &str,
        params: &Value,
        deadline: Instant,
    ) -> std::result::Result<Value, CallFailure> {
        self.next += 1;
        self.send(&json!({"id":self.next,"method":method,"params":params}))
            .map_err(|_| CallFailure::Unavailable)?;
        loop {
            let Some(value) = self.read().map_err(|_| CallFailure::Unavailable)? else {
                pause(deadline).map_err(|_| CallFailure::Unavailable)?;
                continue;
            };
            if value["id"].as_u64() == Some(self.next) {
                return response(&value);
            }
            if self.pending.len() >= 128 {
                return Err(CallFailure::Unavailable);
            }
            if let Some(notice) = notice(&value) {
                self.pending.push_back(notice);
            }
            pause(deadline).map_err(|_| CallFailure::Unavailable)?;
        }
    }

    fn read(&mut self) -> Result<Option<Value>> {
        match self.socket.read() {
            Ok(Message::Text(text)) => serde_json::from_str(&text).map(Some).map_err(failed),
            Ok(Message::Ping(_) | Message::Pong(_)) => Ok(Some(Value::Null)),
            Err(Error::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                Ok(None)
            }
            _ => Err(FAILURE.into()),
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // Best effort and nonblocking: closing the socket also releases the native subscription.
        if self.subscribed {
            let _ = self.send(&json!({"id":self.next + 1,"method":"thread/unsubscribe","params":{"threadId":self.thread}}));
        }
        let _ = self.socket.close(None);
    }
}

fn response(value: &Value) -> std::result::Result<Value, CallFailure> {
    if value.get("error").is_some() {
        return Err(CallFailure::Refused);
    }
    value.get("result").cloned().ok_or(CallFailure::Unavailable)
}

fn failed(_: impl std::fmt::Display) -> String {
    FAILURE.into()
}

fn pause(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(FAILURE.into());
    }
    std::thread::sleep(Duration::from_millis(2));
    Ok(())
}

fn notice(value: &Value) -> Option<Notice> {
    let params = &value["params"];
    let thread = params["threadId"].as_str()?.to_owned();
    match value["method"].as_str()? {
        "thread/tokenUsage/updated" => {
            let turn = params["turnId"].as_str()?.to_owned();
            let mut usage = json!({});
            for part in ["last", "total"] {
                let source = params["tokenUsage"][part].as_object()?;
                let fields = [
                    "inputTokens",
                    "cachedInputTokens",
                    "cacheWriteInputTokens",
                    "outputTokens",
                    "reasoningOutputTokens",
                    "totalTokens",
                ];
                usage[part] = Value::Object(
                    source
                        .iter()
                        .filter(|(key, _)| fields.contains(&key.as_str()))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                );
            }
            Some(Notice::Usage {
                thread,
                turn,
                usage,
            })
        }
        "turn/completed" => Some(Notice::End {
            thread,
            turn: params["turn"]["id"].as_str()?.to_owned(),
        }),
        _ => None,
    }
}
