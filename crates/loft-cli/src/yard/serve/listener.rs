//! The page server's one socket: it listens on the loopback interface alone, takes only the
//! connections it receives there, each on a thread of its own, and opens none. This is the only
//! source of `loft` beside the network client that names a socket, and it never connects.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

/// How long a connection may wait between the bytes of its request.
const READING: Duration = Duration::from_secs(10);

/// A connection the page server accepted.
pub struct Connection(TcpStream);

impl Read for Connection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.read(buffer)
    }
}

impl Write for Connection {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl Connection {
    /// Lets the connection wait for its next bytes as long as it is open, as a stream of
    /// changes does.
    pub fn wait_without_limit(&self) {
        let _ = self.0.set_read_timeout(None);
    }
}

/// The socket, listening on `127.0.0.1:<port>`.
pub struct Listener(TcpListener);

/// Listens on the loopback interface at `port`.
///
/// # Errors
///
/// The port cannot be listened on, as when another program holds it.
pub fn listen(port: u16) -> io::Result<Listener> {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).map(Listener)
}

impl Listener {
    /// Takes each connection received on the loopback interface and hands it to `serve` on a
    /// thread of its own; a connection from anywhere else is closed unread. It returns only where
    /// the socket fails.
    pub fn serve(self, serve: impl Fn(Connection) + Send + Sync + Clone + 'static) {
        for stream in self.0.incoming() {
            let Ok(stream) = stream else {
                continue;
            };
            let loopback = stream.peer_addr().is_ok_and(|peer| peer.ip().is_loopback());
            if !loopback {
                drop(stream);
                continue;
            }
            let _ = stream.set_read_timeout(Some(READING));
            let serve = serve.clone();
            thread::spawn(move || serve(Connection(stream)));
        }
    }
}
