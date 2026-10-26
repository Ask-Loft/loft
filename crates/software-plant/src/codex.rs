//! Local Codex app-server metadata and notification peer. No model or transcript is needed.

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

#[derive(Clone, Default)]
pub struct Scenario {
    pub loaded: Vec<String>,
    pub on_subscribe: Vec<Value>,
    /// Ignore all RPCs, fail subscription, or send malformed JSON at subscription.
    pub silent: bool,
    pub refuse_subscription: bool,
    pub malformed: bool,
}

#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    requests: Mutex<Vec<Value>>,
    outgoing: Mutex<Vec<Message>>,
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
        connected(&mut socket, scenario, shared);
    }
}

type Peer = tungstenite::WebSocket<std::os::unix::net::UnixStream>;

fn connected(socket: &mut Peer, scenario: &Scenario, shared: &Shared) {
    let mut subscribed = false;
    while !shared.stop.load(Ordering::Acquire) {
        if subscribed && !send_pending(socket, shared) {
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

fn send_pending(socket: &mut Peer, shared: &Shared) -> bool {
    let pending = std::mem::take(&mut *shared.outgoing.lock().unwrap());
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
    let response = reply(scenario, method, id);
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

fn reply(scenario: &Scenario, method: &str, id: &Value) -> Value {
    if method == "thread/resume" && scenario.refuse_subscription {
        return json!({"id":id,"error":{"code":-32600,"message":"subscription refused"}});
    }
    let result = match method {
        "thread/loaded/list" => json!({"data":scenario.loaded,"nextCursor":null}),
        "initialize" => json!({"userAgent":"software-plant"}),
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
