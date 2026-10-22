//! The stream of server-sent events an open page listens to: first `hello`, with the page
//! server's version and formats, by which a page served by another version knows to reload its
//! files; then `change` each time the files of a Yard Log served change; and a comment now and
//! then, so that a page gone away is noticed.

use std::io::Write;
use std::sync::atomic::Ordering;

use loft_ports::clock::Clock as _;
use serde_json::json;

use super::Server;
use super::http::write_event_stream_head;
use super::listener::Connection;
use super::live::TICK;
use crate::yard::clock;
use crate::yard::protocol;

/// Ticks between two comments that keep the stream open.
const KEEP: u64 = 60;

/// Streams changes to `connection` until the page goes or the page server stops.
pub fn stream(server: &Server, mut connection: Connection) {
    if write_event_stream_head(&mut connection).is_err() {
        return;
    }
    let hello = json!(protocol::report());
    if send(&mut connection, "hello", &hello.to_string()).is_err() {
        return;
    }
    let clock = clock();
    let mut seen = server.changes.load(Ordering::SeqCst);
    let mut tick: u64 = 0;
    while !server.stopping.load(Ordering::SeqCst) {
        clock.wait(TICK);
        tick += 1;
        let now = server.changes.load(Ordering::SeqCst);
        let sent = if now == seen {
            if tick.is_multiple_of(KEEP) {
                connection
                    .write_all(b": keep\n\n")
                    .and_then(|()| connection.flush())
            } else {
                Ok(())
            }
        } else {
            seen = now;
            send(
                &mut connection,
                "change",
                &json!({ "change": now }).to_string(),
            )
        };
        if sent.is_err() {
            return;
        }
    }
}

fn send(connection: &mut Connection, event: &str, data: &str) -> std::io::Result<()> {
    connection.write_all(format!("event: {event}\ndata: {data}\n\n").as_bytes())?;
    connection.flush()
}
