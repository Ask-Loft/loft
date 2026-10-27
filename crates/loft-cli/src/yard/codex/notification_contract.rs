//! The same supported delivery scenario against the local simulator and native TUI peer.

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use software_plant::codex::{CodexAppServer, Scenario};

use super::{CallFailure, Client};

fn contract(path: &Path, thread: &str, directory: &Path, mut control: impl FnMut(Value) -> Value) {
    let mut client = Client::connect(path, thread).unwrap();
    assert_eq!(client.inspect(directory).unwrap(), None);
    assert!(!client.received("yard-contract-first").unwrap());
    let first = client
        .deliver(
            "yard-contract-first",
            "yard-contract-first\nRead Yard.",
            None,
        )
        .unwrap();
    assert_eq!(
        control(json!({"action":"settle","turn":first}))["status"],
        "completed"
    );
    // Native item history can lag input acceptance; absence is not proof of non-delivery.
    assert!(client.received("yard-contract-first").unwrap());
    let active = control(json!({"action":"active"}))["turn"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(client.inspect(directory).unwrap(), Some(active.clone()));
    assert_eq!(
        client.deliver("stale", "stale", Some("old")),
        Err(CallFailure::Refused)
    );
    assert_eq!(
        client
            .deliver("yard-contract-steer", "yard-contract-steer", Some(&active))
            .unwrap(),
        active
    );
    assert_eq!(
        control(json!({"action":"settle","turn":active}))["status"],
        "completed"
    );
    assert_eq!(client.inspect(directory).unwrap(), None);
    assert_eq!(
        client.deliver("idle", "idle", Some(&active)),
        Err(CallFailure::Refused)
    );
    let raced = control(json!({"action":"active"}))["turn"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        client
            .deliver("yard-contract-race", "yard-contract-race", None)
            .unwrap(),
        raced
    );
    assert_eq!(
        control(json!({"action":"interrupt","turn":raced}))["status"],
        "interrupted"
    );
    assert_eq!(client.inspect(directory).unwrap(), None);
    drop(client);
    let mut reconnected = Client::connect(path, thread).unwrap();
    assert!(reconnected.received("yard-contract-first").unwrap());
}

#[test]
fn simulated_native_notifications_obey_the_shared_delivery_contract() {
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["thread".into()],
        ..Scenario::default()
    });
    contract(
        peer.path(),
        "thread",
        Path::new("/work"),
        |command| match command["action"].as_str().unwrap() {
            "active" => {
                let mut owner = Client::connect(peer.path(), "thread").unwrap();
                json!({"turn":owner.deliver("owner", "owner", None).unwrap()})
            }
            action => {
                let interrupted = action == "interrupt";
                peer.end_turn("thread", interrupted);
                json!({"status":if interrupted {"interrupted"} else {"completed"}})
            }
        },
    );
}

struct Native(Child);
impl Drop for Native {
    fn drop(&mut self) {
        self.0.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(7);
        while self.0.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "requires Codex 0.164.0, Python 3 and a native TUI; uses an isolated loopback model"]
fn real_native_notifications_obey_the_shared_delivery_contract() {
    let mut command = Command::new("python3");
    software_plant::real_git::isolate(&mut command);
    let mut native = Native(
        command
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/codex_wakeup_contract.py"))
            .arg("--controller")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut reader = BufReader::new(native.0.stdout.take().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let ready: Value = serde_json::from_str(&line).unwrap();
    contract(
        Path::new(ready["socket"].as_str().unwrap()),
        ready["thread"].as_str().unwrap(),
        Path::new(ready["directory"].as_str().unwrap()),
        |command| {
            writeln!(native.0.stdin.as_mut().unwrap(), "{command}").unwrap();
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            serde_json::from_str(&line).unwrap()
        },
    );
    native.0.stdin.take();
    assert!(native.0.wait().unwrap().success());
}
