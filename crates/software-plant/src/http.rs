//! A small HTTP/1.1 server on the loopback interface, for simulators of the services Loft calls.
//! It answers one request per connection, from a [`Responder`] a simulator supplies, records
//! every request it reads, and stops when it is dropped. A responder that gives no answer holds
//! the connection open without a word, as a service that never answers does.

use std::fmt::Write as _;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// The largest request this server reads, headers and body together.
const LIMIT: usize = 1 << 20;

/// A request as the server read it.
#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    /// Each header's name in lower case, with its value.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// The value of the header `name`, matched without regard to case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(header, _)| *header == name)
            .map(|(_, value)| value.as_str())
    }
}

/// An answer: a status, its headers and a body.
#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// A response with `status` and an empty body.
    #[must_use]
    pub fn empty(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// A response with `status` and a JSON `body`.
    #[must_use]
    pub fn json(status: u16, body: &serde_json::Value) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
            body: body.to_string().into_bytes(),
        }
    }
}

/// What a simulator answers: a response, or none, which holds the connection open unanswered.
pub trait Responder: Send + 'static {
    fn answer(&mut self, request: &Request) -> Option<Response>;
}

/// A running server.
pub struct Server {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<Request>>>,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    /// Starts a server on a free port of the loopback interface, answering from `responder`.
    ///
    /// # Panics
    ///
    /// No port of the loopback interface can be bound.
    pub fn start(responder: impl Responder) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port binds");
        let address = listener.local_addr().expect("a bound port has an address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stopping = Arc::new(AtomicBool::new(false));
        let thread = {
            let requests = Arc::clone(&requests);
            let stopping = Arc::clone(&stopping);
            thread::spawn(move || serve(&listener, responder, &requests, &stopping))
        };
        Self {
            address,
            requests,
            stopping,
            thread: Some(thread),
        }
    }

    /// The server's address, as `http://127.0.0.1:<port>`.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}", self.address)
    }

    /// Every request read so far, in the order read.
    pub fn requests(&self) -> Vec<Request> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        // A connection wakes the accepting thread, which then sees that it is stopping.
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// An address on the loopback interface where nothing listens, as `http://127.0.0.1:<port>`: a
/// connection to it is refused.
///
/// # Panics
///
/// No port of the loopback interface can be bound.
#[must_use]
pub fn refusing_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port binds");
    let address = listener.local_addr().expect("a bound port has an address");
    drop(listener);
    format!("http://{address}")
}

fn serve(
    listener: &TcpListener,
    mut responder: impl Responder,
    requests: &Mutex<Vec<Request>>,
    stopping: &AtomicBool,
) {
    // Connections left unanswered stay open until the server stops.
    let mut held = Vec::new();
    for stream in listener.incoming() {
        if stopping.load(Ordering::SeqCst) {
            break;
        }
        let Ok(mut stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let Ok(request) = read(&mut stream) else {
            continue;
        };
        let answer = responder.answer(&request);
        requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request);
        match answer {
            Some(response) => {
                let _ = write(&mut stream, &response);
                let _ = stream.shutdown(Shutdown::Both);
            }
            None => held.push(stream),
        }
    }
}

fn read(stream: &mut TcpStream) -> io::Result<Request> {
    let mut reader = BufReader::new(stream.take(LIMIT as u64));
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (Some(method), Some(path)) = (parts.next(), parts.next()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "no request line",
        ));
    };
    let (method, path) = (method.to_owned(), path.to_owned());
    let mut headers = Vec::new();
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length.min(LIMIT)];
    reader.read_exact(&mut body)?;
    Ok(Request {
        method,
        path,
        headers,
        body,
    })
}

fn write(stream: &mut TcpStream, response: &Response) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        reason(response.status)
    );
    for (name, value) in &response.headers {
        let _ = write!(head, "{name}: {value}\r\n");
    }
    let _ = write!(
        head,
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        response.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

const fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}
