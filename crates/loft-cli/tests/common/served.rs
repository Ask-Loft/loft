//! A page server run by `loft yard serve` for a test, in front of the git simulator, and a small
//! HTTP client that speaks to it over the loopback interface. The client lives in the tests, not
//! in `loft`, which opens no connection of its own but through its network client.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Child;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::yard::Yard;

/// A port on the loopback interface that nothing listens on now.
pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A running page server, stopped when the test drops it.
pub struct Served {
    pub child: Child,
    pub port: u16,
    pub home: PathBuf,
    /// The document `loft yard serve` printed once it served.
    pub started: Value,
}

/// A response: its status, its head and its body.
pub struct Reply {
    pub status: u16,
    pub head: String,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&self.body)))
    }
}

/// The first line `child` prints on its standard output, as JSON, within 10 seconds.
pub fn first_document(child: &mut Child) -> Value {
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(stdout).read_line(&mut line);
        let _ = sender.send(line);
    });
    let Ok(line) = receiver.recv_timeout(Duration::from_secs(10)) else {
        let _ = child.kill();
        panic!("`loft yard serve` printed nothing within 10 seconds");
    };
    serde_json::from_str(&line).unwrap_or_else(|_| panic!("not a document: {line:?}"))
}

impl Served {
    /// Starts `loft yard serve` on a free port in `yard`'s tree, with `extra` arguments, and waits
    /// until it serves.
    pub fn start(yard: &Yard, extra: &[&str]) -> Self {
        Self::start_on(yard, free_port(), extra)
    }

    /// Starts `loft yard serve` on `port`.
    pub fn start_on(yard: &Yard, port: u16, extra: &[&str]) -> Self {
        let port_text = port.to_string();
        let mut args = vec!["serve", "--port", &port_text];
        args.extend_from_slice(extra);
        let mut child = yard.spawn(&args);
        let started = first_document(&mut child);
        assert_eq!(started["outcome"], "serving", "{started}");
        Self {
            child,
            port,
            home: yard.scratch.home(),
            started,
        }
    }

    /// The control token the page server keeps in `~/.yard/token`.
    pub fn token(&self) -> String {
        fs::read_to_string(self.home.join(".yard/token")).unwrap()
    }

    /// Sends `request`, whose head is complete but for its end, and reads the whole response.
    pub fn send(&self, head: &str, body: &[u8]) -> Reply {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let request = format!(
            "{head}content-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(request.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        let mut whole = Vec::new();
        stream.read_to_end(&mut whole).unwrap();
        reply(&whole)
    }

    /// `GET path`, addressed to the page server.
    pub fn get(&self, path: &str) -> Reply {
        self.send(
            &format!("GET {path} HTTP/1.1\r\nhost: 127.0.0.1:{}\r\n", self.port),
            b"",
        )
    }

    /// `POST path` with `body` and the control token, as the mod sends it.
    pub fn post_as_mod(&self, path: &str, body: &Value) -> Reply {
        let head = format!(
            "POST {path} HTTP/1.1\r\nhost: 127.0.0.1:{}\r\nauthorization: Bearer {}\r\n",
            self.port,
            self.token()
        );
        self.send(&head, body.to_string().as_bytes())
    }

    /// `POST path` with `body`, from the page the page server served.
    pub fn post_as_page(&self, path: &str, body: &Value) -> Reply {
        let head = format!(
            "POST {path} HTTP/1.1\r\nhost: 127.0.0.1:{port}\r\norigin: http://127.0.0.1:{port}\r\n",
            port = self.port
        );
        self.send(&head, body.to_string().as_bytes())
    }

    /// Waits up to `limit` for the page server's process to end, and says whether it did.
    pub fn ends_within(&mut self, limit: Duration) -> bool {
        let began = Instant::now();
        while began.elapsed() < limit {
            if self.child.try_wait().unwrap().is_some() {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        false
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn reply(whole: &[u8]) -> Reply {
    let split = whole
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("no head in {:?}", String::from_utf8_lossy(whole)));
    let head = String::from_utf8_lossy(&whole[..split]).into_owned();
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|status| status.parse().ok())
        .unwrap();
    Reply {
        status,
        head,
        body: whole[split + 4..].to_vec(),
    }
}

/// Waits up to `limit` for `done` to hold, and says whether it did.
pub fn within(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let began = Instant::now();
    while began.elapsed() < limit {
        if done() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    done()
}
