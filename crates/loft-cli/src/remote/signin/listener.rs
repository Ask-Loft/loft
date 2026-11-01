//! The loopback listener a sign-in's authorization request redirects to: a port of `127.0.0.1`
//! that the system gives, which answers `/callback` with the `state` asked for and nothing else.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

use crate::out::errln;
use crate::percent::parameter;

/// What the browser brought back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Callback {
    Code(String),
    Error(String),
}

/// How a wait ended without a callback.
#[derive(Debug)]
pub enum Waited {
    TimedOut,
    Failed(io::Error),
}

/// The page the browser shows once the callback arrives.
const PAGE: &str = "<!doctype html><meta charset=\"utf-8\"><title>Loft</title>\
                    <p>Loft has the answer. You may close this page and return to the terminal.</p>";

pub struct Listener {
    listener: TcpListener,
    port: u16,
}

impl Listener {
    /// Listens on a port of `127.0.0.1` that the system gives.
    ///
    /// # Errors
    ///
    /// No port can be bound.
    pub fn bind() -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        Ok(Self { listener, port })
    }

    /// The redirect URI on the port it listens on.
    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.port)
    }

    /// Waits up to `limit` for a callback with `state`, answering every other request as it
    /// comes. Where `browser` ends in failure first, says once that the address is to be opened
    /// by hand, and goes on waiting.
    ///
    /// # Errors
    ///
    /// No callback arrived within `limit`, or the port failed.
    pub fn wait(
        &self,
        state: &str,
        limit: Duration,
        mut browser: Option<Child>,
    ) -> Result<Callback, Waited> {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            let callback = match self.listener.accept() {
                Ok((stream, _)) => answer(&stream, state),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    watch(&mut browser);
                    thread::sleep(Duration::from_millis(20));
                    None
                }
                // A connection reset or aborted before it was taken ends nothing.
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionAborted
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::Interrupted
                    ) =>
                {
                    None
                }
                Err(error) => return Err(Waited::Failed(error)),
            };
            if let Some(callback) = callback {
                return Ok(callback);
            }
        }
        Err(Waited::TimedOut)
    }
}

/// Says once that the address is to be opened by hand, where the browser ended in failure.
fn watch(browser: &mut Option<Child>) {
    let Some(child) = browser else { return };
    if let Ok(Some(status)) = child.try_wait() {
        if !status.success() {
            errln!("No browser could be opened: open the address above.");
        }
        *browser = None;
    }
}

/// Answers one request, and gives the callback it brought, if it is the one asked for.
fn answer(stream: &TcpStream, state: &str) -> Option<Callback> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut header = String::new();
    while reader.read_line(&mut header).is_ok_and(|read| read > 2) {
        header.clear();
    }
    let target = line.split_whitespace().nth(1).unwrap_or_default();
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (status, body, callback) = if path != "/callback" {
        ("404 Not Found", "Not found.", None)
    } else if parameter(query, "state").as_deref() != Some(state) {
        (
            "400 Bad Request",
            "This is not the sign-in loft is waiting for.",
            None,
        )
    } else if let Some(error) = parameter(query, "error") {
        ("200 OK", PAGE, Some(Callback::Error(error)))
    } else {
        let code = parameter(query, "code").unwrap_or_default();
        ("200 OK", PAGE, Some(Callback::Code(code)))
    };
    let mut stream = stream;
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    callback
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use super::*;

    fn visit(port: u16, target: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(stream, "GET {target} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut answer = String::new();
        let _ = stream.read_to_string(&mut answer);
        answer
    }

    #[test]
    fn a_wait_without_a_callback_ends_at_its_limit() {
        let listener = Listener::bind().unwrap();
        let started = Instant::now();
        let waited = listener.wait("s", Duration::from_millis(200), None);
        assert!(matches!(waited, Err(Waited::TimedOut)));
        assert!(started.elapsed() >= Duration::from_millis(200));
    }

    #[test]
    fn only_the_callback_with_the_state_asked_for_ends_the_wait() {
        let listener = Listener::bind().unwrap();
        let port = listener.port;
        let visitor = thread::spawn(move || {
            [
                visit(port, "/elsewhere"),
                visit(port, "/callback?state=other&code=x"),
                visit(port, "/callback?state=s&code=c%2B1"),
            ]
        });
        let callback = listener.wait("s", Duration::from_secs(10), None).unwrap();
        assert_eq!(callback, Callback::Code("c+1".to_owned()));
        let answers = visitor.join().unwrap();
        assert!(answers[0].starts_with("HTTP/1.1 404"), "{}", answers[0]);
        assert!(answers[1].starts_with("HTTP/1.1 400"), "{}", answers[1]);
        assert!(answers[2].starts_with("HTTP/1.1 200"), "{}", answers[2]);
    }
}
