//! Who the page server answers: a request addressed to it as `localhost` or by the address of the
//! machine it reached alone; a request
//! that changes Yard Log, the trees served or a session's events only from the page it served or
//! with the control token; a request that only reads with neither. A refused request changes no
//! file.

use std::net::SocketAddr;

use serde_json::{Value, json};

use crate::served::{Served, machine_address};
use crate::snapshot::snapshot;
use crate::yard::{ASK, Yard};

/// Yard Log with `q1` sent and a page server serving its tree.
fn asked() -> (Yard, Served) {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let tree = yard.scratch.0.display().to_string();
    let served = Served::start(&yard, &["--tree", &tree]);
    (yard, served)
}

/// Yard Log with `q1` sent and a page server serving its tree on every address of the machine.
fn asked_on_every_address() -> (Yard, Served) {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let tree = yard.scratch.0.display().to_string();
    let served = Served::start_on_every_address(&yard, &["--tree", &tree]);
    (yard, served)
}

/// The page's answer to `q1`.
fn answer(yard: &Yard) -> Value {
    json!({
        "tree": yard.scratch.0.canonicalize().unwrap().display().to_string(),
        "branch": crate::yard::BRANCH,
        "answers": [{ "block": "q1", "labels": ["Top"] }],
    })
}

/// Sends `body` to `path` with `head`'s lines after the request line.
fn post(served: &Served, path: &str, lines: &str, body: &Value) -> crate::served::Reply {
    served.send(
        &format!("POST {path} HTTP/1.1\r\n{lines}"),
        body.to_string().as_bytes(),
    )
}

#[test]
fn a_request_with_another_host_is_refused_and_changes_no_file() {
    let (yard, served) = asked();
    let before = snapshot(&yard.scratch.0);
    let lines = format!(
        "host: elsewhere.example:{port}\r\norigin: http://127.0.0.1:{port}\r\n",
        port = served.port
    );
    let refused = post(&served, "/api/answer", &lines, &answer(&yard));
    assert_eq!(refused.status, 403);
    assert_eq!(refused.json()["errors"][0]["kind"], "host");
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
    let read = served.send(
        "GET /api/version HTTP/1.1\r\nhost: elsewhere.example\r\n",
        b"",
    );
    assert_eq!(read.status, 403);
}

#[test]
fn a_request_addressed_to_the_address_of_the_machine_it_reached_is_answered() {
    let (_yard, served) = asked_on_every_address();
    let machine = machine_address();
    assert_eq!(served.version_at(machine).status, 200);
    let elsewhere = served.send(
        &format!(
            "GET /api/version HTTP/1.1\r\nhost: {}\r\n",
            SocketAddr::new(machine, served.port)
        ),
        b"",
    );
    assert_eq!(
        elsewhere.status, 403,
        "an address the request did not reach"
    );
}

#[test]
fn a_request_naming_the_machine_by_a_name_other_than_localhost_is_refused() {
    let (_yard, served) = asked();
    let read = served.send(
        &format!(
            "GET /api/version HTTP/1.1\r\nhost: machine.example.ts.net:{}\r\n",
            served.port
        ),
        b"",
    );
    assert_eq!(read.status, 403);
    assert_eq!(read.json()["errors"][0]["kind"], "host");
    let local = served.send(
        &format!(
            "GET /api/version HTTP/1.1\r\nhost: localhost:{}\r\n",
            served.port
        ),
        b"",
    );
    assert_eq!(local.status, 200);
}

#[test]
fn a_change_from_the_page_opened_at_another_address_is_accepted_without_the_control_token() {
    let (yard, served) = asked_on_every_address();
    let machine = SocketAddr::new(machine_address(), served.port);
    let head =
        format!("POST /api/answer HTTP/1.1\r\nhost: {machine}\r\norigin: http://{machine}\r\n");
    let answered = served.send_at(machine.ip(), &head, answer(&yard).to_string().as_bytes());
    assert_eq!(
        answered.status,
        200,
        "{}",
        String::from_utf8_lossy(&answered.body)
    );
    assert_eq!(answered.json()["outcome"], "answered");
}

#[test]
fn a_change_from_another_origin_is_refused_and_changes_no_file() {
    let (yard, served) = asked();
    let before = snapshot(&yard.scratch.0);
    let lines = format!(
        "host: 127.0.0.1:{}\r\norigin: http://elsewhere.example\r\nauthorization: Bearer {}\r\n",
        served.port,
        served.token()
    );
    let refused = post(&served, "/api/answer", &lines, &answer(&yard));
    assert_eq!(refused.status, 403);
    assert_eq!(refused.json()["errors"][0]["kind"], "origin");
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

#[test]
fn a_change_from_the_page_the_page_server_served_is_accepted() {
    let (yard, served) = asked();
    let answered = served.post_as_page("/api/answer", &answer(&yard));
    assert_eq!(
        answered.status,
        200,
        "{}",
        String::from_utf8_lossy(&answered.body)
    );
    assert_eq!(answered.json()["outcome"], "answered");
}

#[test]
fn a_change_with_no_origin_and_no_token_is_refused_and_changes_no_file() {
    let (yard, served) = asked();
    let before = snapshot(&yard.scratch.0);
    let lines = format!("host: 127.0.0.1:{}\r\n", served.port);
    let refused = post(&served, "/api/answer", &lines, &answer(&yard));
    assert_eq!(refused.status, 401);
    assert_eq!(refused.json()["errors"][0]["kind"], "token");
    let wrong = format!(
        "host: 127.0.0.1:{}\r\nauthorization: Bearer not-the-token\r\n",
        served.port
    );
    assert_eq!(post(&served, "/api/events", &wrong, &json!([])).status, 401);
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

#[test]
fn a_change_with_no_origin_and_the_control_token_is_accepted() {
    let (yard, served) = asked();
    let answered = served.post_as_mod("/api/answer", &answer(&yard));
    assert_eq!(answered.status, 200);
    let registered = served.post_as_mod(
        "/api/register",
        &json!({ "tree": yard.scratch.0.display().to_string() }),
    );
    assert_eq!(registered.status, 200);
}

#[test]
fn a_request_that_only_reads_needs_neither_an_origin_nor_the_token() {
    let (_yard, served) = asked();
    for path in ["/", "/api/version", "/api/trees"] {
        assert_eq!(served.get(path).status, 200, "{path}");
    }
}

#[test]
fn the_control_token_is_made_once_and_readable_only_by_its_owner() {
    let yard = Yard::new();
    let token = yard.scratch.home().join(".yard/token");
    assert!(!token.exists());
    let first = Served::start(&yard, &[]).token();
    assert!(first.len() >= 32, "{first}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&token).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let again = Served::start(&yard, &[]).token();
    assert_eq!(again, first);
}
