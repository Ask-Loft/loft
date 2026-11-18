//! The page server's sockets: it listens on every address of the machine, or on each address it
//! is given and on the loopback interface, takes each connection it receives on a thread of its
//! own, and opens none. This is the only source of `loft` beside the network client and a
//! sign-in's listener that names a socket, and it never connects.

use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use loft_ports::clock::Clock;

/// How long a connection may wait between the bytes of its request.
const READING: Duration = Duration::from_secs(10);

/// How long the page server waits before it tries again an address the machine does not have.
const AGAIN: Duration = Duration::from_secs(1);

/// An address that `--listen` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Address(IpAddr);

/// Reads an address that `--listen` names, which is an IP address and nothing else.
///
/// # Errors
///
/// `text` is not an IP address.
pub fn parse_address(text: &str) -> Result<Address, String> {
    text.parse::<IpAddr>()
        .map(Address)
        .map_err(|_| format!("`{text}` is not an IP address"))
}

/// A connection the page server accepted.
pub struct Connection {
    stream: TcpStream,
    /// The address of the machine the connection reached.
    reached: Option<SocketAddr>,
}

impl Read for Connection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buffer)
    }
}

impl Write for Connection {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

impl Connection {
    /// Lets the connection wait for its next bytes as long as it is open, as a stream of
    /// changes does.
    pub fn wait_without_limit(&self) {
        let _ = self.stream.set_read_timeout(None);
    }

    /// The address of the machine the connection reached, as a `Host` names it, such as
    /// `192.168.1.5:7411` or `[fd7a::1]:7411`.
    pub fn reached(&self) -> Option<String> {
        self.reached.map(|address| address.to_string())
    }
}

/// The sockets the page server listens on, and the addresses it waits for the machine to have.
pub struct Listener {
    sockets: Vec<TcpListener>,
    missing: Vec<SocketAddr>,
}

/// Listens at `port` on every address of the machine where `addresses` is empty or names
/// `0.0.0.0`, and otherwise on each of `addresses` and on the loopback interface. An address the
/// machine does not have yet is left for [`Listener::serve`] to try again.
///
/// # Errors
///
/// The port cannot be listened on, as when another program holds it.
pub fn listen(port: u16, addresses: &[Address]) -> io::Result<Listener> {
    let loopback = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let every = addresses.is_empty() || addresses.iter().any(|named| named.0.is_unspecified());
    if every {
        let any = addresses
            .iter()
            .find(|named| named.0.is_unspecified())
            .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |named| named.0);
        // Some platforms let a program listen on every address beside one that holds the
        // loopback address alone; listening there first finds that program.
        drop(bind(loopback)?);
        return Ok(Listener {
            sockets: vec![bind(SocketAddr::new(any, port))?],
            missing: Vec::new(),
        });
    }
    let mut sockets = vec![bind(loopback)?];
    let mut missing = Vec::new();
    let mut named: Vec<SocketAddr> = Vec::new();
    for address in addresses.iter().map(|named| SocketAddr::new(named.0, port)) {
        if address != loopback && !named.contains(&address) {
            named.push(address);
        }
    }
    for address in named {
        match bind(address) {
            Ok(socket) => sockets.push(socket),
            Err(error) if error.kind() == io::ErrorKind::AddrNotAvailable => missing.push(address),
            Err(error) => return Err(error),
        }
    }
    Ok(Listener { sockets, missing })
}

/// The one place the page server takes a socket.
fn bind(address: SocketAddr) -> io::Result<TcpListener> {
    TcpListener::bind(address)
}

impl Listener {
    /// Takes each connection received on any of its sockets and hands it to `serve` on a thread
    /// of its own, and tries each address the machine did not have again every second, through
    /// `clock`, until it listens there too. It returns only where the first socket fails.
    pub fn serve(
        self,
        clock: &'static (impl Clock + Sync),
        serve: impl Fn(Connection) + Send + Sync + Clone + 'static,
    ) {
        let Self { sockets, missing } = self;
        if !missing.is_empty() {
            let serve = serve.clone();
            thread::spawn(move || {
                take_missing(missing, bind, clock, |socket| accept_apart(socket, &serve));
            });
        }
        let mut sockets = sockets.into_iter();
        let Some(first) = sockets.next() else {
            return;
        };
        for socket in sockets {
            accept_apart(socket, &serve);
        }
        accept(&first, &serve);
    }
}

/// Takes the connections `socket` receives on a thread of its own.
fn accept_apart(
    socket: TcpListener,
    serve: &(impl Fn(Connection) + Send + Sync + Clone + 'static),
) {
    let serve = serve.clone();
    thread::spawn(move || accept(&socket, &serve));
}

/// Hands each connection `socket` receives to `serve` on a thread of its own.
fn accept(socket: &TcpListener, serve: &(impl Fn(Connection) + Send + Sync + Clone + 'static)) {
    for stream in socket.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let _ = stream.set_read_timeout(Some(READING));
        let reached = stream.local_addr().ok();
        let serve = serve.clone();
        thread::spawn(move || serve(Connection { stream, reached }));
    }
}

/// Tries each of `missing` again, every [`AGAIN`] as `clock` waits, until `bind` listens there,
/// and hands each socket it takes to `taken`.
fn take_missing(
    mut missing: Vec<SocketAddr>,
    mut bind: impl FnMut(SocketAddr) -> io::Result<TcpListener>,
    clock: &impl Clock,
    mut taken: impl FnMut(TcpListener),
) {
    while !missing.is_empty() {
        clock.wait(AGAIN);
        missing.retain(|address| match bind(*address) {
            Ok(socket) => {
                taken(socket);
                false
            }
            Err(_) => true,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use loft_ports::PortError;

    use super::*;

    /// A clock whose time passes only as it waits.
    struct Waiting(Cell<Duration>);

    impl Clock for Waiting {
        fn now(&self) -> Result<u64, PortError> {
            Ok(0)
        }

        fn elapsed(&self) -> Duration {
            self.0.get()
        }

        fn wait(&self, length: Duration) {
            self.0.set(self.0.get() + length);
        }
    }

    #[test]
    fn an_address_the_machine_gains_is_listened_on_within_5_seconds() {
        let appears = Duration::from_millis(3_500);
        let clock = Waiting(Cell::new(Duration::ZERO));
        let gained = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 7411);
        let mut tries = 0;
        let mut taken_at = None;
        take_missing(
            vec![gained],
            |address| {
                assert_eq!(address, gained);
                tries += 1;
                if clock.elapsed() < appears {
                    return Err(io::Error::from(io::ErrorKind::AddrNotAvailable));
                }
                bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            },
            &clock,
            |_| taken_at = Some(clock.elapsed()),
        );
        let taken_at = taken_at.expect("the address is listened on");
        let after = taken_at
            .checked_sub(appears)
            .expect("the address is not listened on before it appears");
        assert!(after <= Duration::from_secs(5), "{taken_at:?}");
        assert!(tries > 1, "the address was tried again");
    }

    #[test]
    fn an_ip_address_is_read_and_anything_else_is_refused_by_name() {
        assert_eq!(
            parse_address("100.76.101.57"),
            Ok(Address(IpAddr::V4(Ipv4Addr::new(100, 76, 101, 57))))
        );
        assert!(parse_address("fd7a:115c:a1e0::1").is_ok());
        let refused = parse_address("machine.example.ts.net").unwrap_err();
        assert!(refused.contains("machine.example.ts.net"), "{refused}");
    }
}
