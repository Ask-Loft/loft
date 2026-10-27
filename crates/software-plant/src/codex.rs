//! Local Codex app-server metadata and notification peer. No model or transcript is needed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{Value, json};
use tungstenite::{Error, Message, accept};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[path = "codex_turns.rs"]
mod turns;

#[derive(Clone)]
pub struct NativeThread {
    pub directory: String,
    pub active_turn: Option<String>,
    pub items: Vec<Value>,
}

impl Default for NativeThread {
    fn default() -> Self {
        Self {
            directory: "/work".into(),
            active_turn: None,
            items: Vec::new(),
        }
    }
}

#[derive(Clone, Default)]
pub struct Scenario {
    pub loaded: Vec<String>,
    pub on_subscribe: Vec<Value>,
    /// Account metadata for the non-refreshing lookup; no real credentials are used.
    pub account: Option<Value>,
    /// Ignore all RPCs, fail subscription, or send malformed JSON at subscription.
    pub silent: bool,
    pub refuse_subscription: bool,
    pub malformed: bool,
    pub threads: BTreeMap<String, NativeThread>,
    pub refuse_methods: Vec<String>,
    /// Apply a request but close its first connection before acknowledging it.
    pub drop_acknowledgements: Vec<String>,
}

#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    requests: Mutex<Vec<Value>>,
    outgoing: Mutex<Vec<Message>>,
    threads: Mutex<BTreeMap<String, NativeThread>>,
    dropped: Mutex<BTreeSet<String>>,
    turns: AtomicU64,
}

pub struct CodexAppServer {
    path: PathBuf,
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl CodexAppServer {
    pub fn start(scenario: Scenario) -> Self {
        // Darwin limits Unix socket paths to 104 bytes; keep this independent of scratch roots.
        let path = PathBuf::from(format!(
            "/tmp/loft-codex-{}-{}.sock",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let listener = UnixListener::bind(&path).expect("bind simulator socket");
        listener.set_nonblocking(true).unwrap();
        let shared = Arc::new(Shared::default());
        *shared.threads.lock().unwrap() = scenario
            .loaded
            .iter()
            .map(|id| {
                (
                    id.clone(),
                    scenario.threads.get(id).cloned().unwrap_or_default(),
                )
            })
            .collect();
        let running = Arc::clone(&shared);
        let worker = thread::spawn(move || serve(&listener, &scenario, &running));
        Self {
            path,
            shared,
            worker: Some(worker),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn requests(&self) -> Vec<Value> {
        self.shared.requests.lock().unwrap().clone()
    }

    pub fn emit(&self, value: &Value) {
        self.shared
            .outgoing
            .lock()
            .unwrap()
            .push(Message::Text(value.to_string().into()));
    }

    pub fn raw(&self, text: String) {
        self.shared
            .outgoing
            .lock()
            .unwrap()
            .push(Message::Text(text.into()));
    }

    pub fn disconnect(&self) {
        self.shared
            .outgoing
            .lock()
            .unwrap()
            .push(Message::Close(None));
    }

    pub fn end_turn(&self, thread: &str, interrupted: bool) {
        let mut threads = self.shared.threads.lock().unwrap();
        if let Some(turn) = threads
            .get_mut(thread)
            .and_then(|state| state.active_turn.take())
        {
            self.emit(&json!({"method":"turn/completed","params":{"threadId":thread,"turn":{"id":turn,"status":if interrupted {"interrupted"} else {"completed"}}}}));
        }
    }
}

impl Drop for CodexAppServer {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
        let _ = fs::remove_file(&self.path);
    }
}

fn serve(listener: &UnixListener, scenario: &Scenario, shared: &Shared) {
    thread::scope(|scope| {
        while !shared.stop.load(Ordering::Acquire) {
            let Ok((stream, _)) = listener.accept() else {
                thread::sleep(Duration::from_millis(2));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(50)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_millis(50)))
                .unwrap();
            let Ok(mut socket) = accept(stream) else {
                continue;
            };
            scope.spawn(move || connected(&mut socket, scenario, shared));
        }
    });
}

type Peer = tungstenite::WebSocket<std::os::unix::net::UnixStream>;

fn connected(socket: &mut Peer, scenario: &Scenario, shared: &Shared) {
    let mut subscribed = false;
    let mut cursor = shared.outgoing.lock().unwrap().len();
    while !shared.stop.load(Ordering::Acquire) {
        if subscribed && !send_pending(socket, shared, &mut cursor) {
            break;
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                if !requested(socket, &text, scenario, shared, &mut subscribed) {
                    break;
                }
            }
            Err(Error::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }
}

fn send_pending(socket: &mut Peer, shared: &Shared, cursor: &mut usize) -> bool {
    let outgoing = shared.outgoing.lock().unwrap();
    let pending = outgoing[*cursor..].to_vec();
    *cursor = outgoing.len();
    drop(outgoing);
    pending
        .into_iter()
        .all(|message| socket.send(message).is_ok())
}

fn requested(
    socket: &mut Peer,
    text: &str,
    scenario: &Scenario,
    shared: &Shared,
    subscribed: &mut bool,
) -> bool {
    let Ok(request) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    shared.requests.lock().unwrap().push(request.clone());
    if scenario.silent {
        return true;
    }
    let Some(id) = request.get("id") else {
        return true;
    };
    let method = request["method"].as_str().unwrap_or_default();
    let response = reply(scenario, method, id, &request["params"], shared);
    if scenario
        .drop_acknowledgements
        .iter()
        .any(|name| name == method)
        && shared.dropped.lock().unwrap().insert(method.into())
    {
        return false;
    }
    if socket
        .send(Message::Text(response.to_string().into()))
        .is_err()
    {
        return false;
    }
    if method == "thread/resume" && !scenario.refuse_subscription {
        *subscribed = true;
        return send_initial(socket, scenario);
    }
    if method == "thread/unsubscribe" {
        *subscribed = false;
    }
    true
}

fn reply(scenario: &Scenario, method: &str, id: &Value, params: &Value, shared: &Shared) -> Value {
    if (method == "thread/resume" && scenario.refuse_subscription)
        || scenario.refuse_methods.iter().any(|name| name == method)
    {
        return json!({"id":id,"error":{"code":-32600,"message":"subscription refused"}});
    }
    let result = match method {
        "thread/loaded/list" => json!({"data":scenario.loaded,"nextCursor":null}),
        "initialize" => json!({"userAgent":"software-plant"}),
        "account/read" => {
            json!({"account":scenario.account,"requiresOpenaiAuth":scenario.account.is_some()})
        }
        "thread/read" | "thread/turns/list" | "thread/items/list" | "turn/start" | "turn/steer" => {
            return match turns::reply(shared, method, params) {
                Some(result) => json!({"id":id,"result":result}),
                None => {
                    json!({"id":id,"error":{"code":-32600,"message":"thread or expected turn unavailable"}})
                }
            };
        }
        _ => json!({}),
    };
    json!({"id":id,"result":result})
}

fn send_initial(
    socket: &mut tungstenite::WebSocket<std::os::unix::net::UnixStream>,
    scenario: &Scenario,
) -> bool {
    if scenario.malformed && socket.send(Message::Text("{bad json".into())).is_err() {
        return false;
    }
    scenario.on_subscribe.iter().all(|notice| {
        socket
            .send(Message::Text(notice.to_string().into()))
            .is_ok()
    })
}
