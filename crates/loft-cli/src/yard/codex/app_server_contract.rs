//! The same supported usage/subscription scenario against the plant and real native peer.

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use software_plant::codex::{CodexAppServer, Scenario};

use super::{Connection, Notice};

fn contract(path: &Path, thread: &str, mut start: impl FnMut(u64)) {
    for count in 2..=3 {
        let mut connection = Connection::connect(path, thread).unwrap();
        // Reconnecting imports no prior request, even though the thread remains loaded.
        let until = Instant::now() + Duration::from_millis(100);
        while Instant::now() < until {
            assert_eq!(connection.poll().unwrap(), []);
            std::thread::sleep(Duration::from_millis(5));
        }
        start(count);
        let began = Instant::now();
        let mut observed = false;
        let mut ended = false;
        while !ended {
            for notice in connection.poll().unwrap() {
                let (usage, end) = verify_notice(&notice, thread, count);
                observed |= usage;
                ended |= end;
            }

            assert!(began.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(observed);
    }
}

fn verify_notice(notice: &Notice, thread: &str, count: u64) -> (bool, bool) {
    match notice {
        Notice::Usage {
            thread: actual,
            usage,
            ..
        } => {
            assert_eq!(actual, thread);
            assert_eq!(usage["last"]["inputTokens"], 100);
            assert_eq!(usage["last"]["cachedInputTokens"], 30);
            assert_eq!(usage["last"]["outputTokens"], 5);
            assert_eq!(usage["last"]["reasoningOutputTokens"], 2);
            assert_eq!(usage["total"]["inputTokens"], count * 100);
            (true, false)
        }
        Notice::End { thread: actual, .. } => {
            assert_eq!(actual, thread);
            (false, true)
        }
    }
}

#[test]
fn simulated_codex_subscription_and_usage_obey_the_supported_contract() {
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["thread".into()],
        ..Scenario::default()
    });
    contract(peer.path(), "thread", |count| {
        let last = json!({"inputTokens":100,"cachedInputTokens":30,"outputTokens":5,"reasoningOutputTokens":2,"totalTokens":105});
        let mut total = last.clone();
        total["inputTokens"] = json!(count * 100);
        total["cachedInputTokens"] = json!(count * 30);
        total["outputTokens"] = json!(count * 5);
        total["totalTokens"] = json!(count * 105);
        peer.emit(&json!({"method":"thread/tokenUsage/updated","params":{"threadId":"thread","turnId":format!("turn-{count}"),"tokenUsage":{"last":last,"total":total}}}));
        peer.emit(&json!({"method":"turn/completed","params":{"threadId":"thread","turn":{"id":format!("turn-{count}")}}}));
    });
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
#[ignore = "requires Codex 0.164.0 and Python 3; uses an isolated home and loopback provider"]
fn real_codex_subscription_and_usage_obey_the_supported_contract_without_an_account() {
    let mut command = Command::new("python3");
    software_plant::real_git::isolate(&mut command);
    let mut native = Native(
        command
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/codex_app_server_contract.py"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut ready = String::new();
    BufReader::new(native.0.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    let ready: Value = serde_json::from_str(&ready).unwrap();
    contract(
        Path::new(ready["socket"].as_str().unwrap()),
        ready["thread"].as_str().unwrap(),
        |_| {
            native
                .0
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"turn\n")
                .unwrap();
        },
    );
    native.0.stdin.take();
    assert!(native.0.wait().unwrap().success());
}
