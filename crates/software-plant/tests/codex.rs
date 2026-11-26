//! The Codex app server simulator sets up one accepted connection at a time, and goes on serving
//! whatever a peer does before its connection is set up.

use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde_json::{Value, json};
use software_plant::codex::{CodexAppServer, Scenario};
use tungstenite::{Message, client};

#[test]
fn a_peer_gone_before_its_connection_is_set_up_leaves_the_next_connection_served() {
    let server = CodexAppServer::start(Scenario::default());
    // A peer that sends nothing holds the simulator in its handshake until the read times out,
    // so the peer accepted after it has closed before the simulator sets its connection up.
    let holding = UnixStream::connect(server.path()).unwrap();
    drop(UnixStream::connect(server.path()).unwrap());
    let stream = UnixStream::connect(server.path()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let (mut socket, _) = client("ws://localhost/", stream).expect("the next handshake");
    let request = json!({"id":1,"method":"initialize","params":{}});
    socket
        .send(Message::Text(request.to_string().into()))
        .unwrap();
    let Message::Text(reply) = socket.read().unwrap() else {
        panic!("a text reply");
    };
    assert_eq!(
        serde_json::from_str::<Value>(&reply).unwrap(),
        json!({"id":1,"result":{"userAgent":"software-plant"}})
    );
    assert_eq!(server.requests(), [request]);
    drop(holding);
}
