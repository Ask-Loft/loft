//! The page server's sockets: it listens on the loopback interface and on each address it is
//! given, takes each connection it receives on a thread of its own, and opens none. This is the
//! only source of `loft` beside the network client and a sign-in's listener that names a socket,
//! and it never connects.

use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use loft_ports::clock::Clock;

/// How long a connection may wait between the bytes of its request.
const READING: Duration = Duration::from_secs(10);

/// Short enough that an address the machine gains is listened on within the 5 seconds the page
/// server allows.
const AGAIN: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Address(IpAddr);

pub fn parse_address(text: &str) -> Result<Address, String> {
    text.parse::<IpAddr>()
        .map(Address)
        .map_err(|_| format!("`{text}` is not an IP address"))
}

/// The addresses a page server given `addresses` listens on, as `~/.yard/server.json` names them
/// to compare one page server with another: the loopback address, which every one listens on,
/// left out, and every other address left out beside `0.0.0.0`.
pub fn recorded(addresses: &[Address]) -> Vec<String> {
    let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mut named: Vec<IpAddr> = match every_address(addresses) {
        Some(every) => vec![every],
        None => addresses
            .iter()
            .map(|named| named.0)
            .filter(|address| *address != loopback)
            .collect(),
    };
    named.sort_unstable();
    named.dedup();
    named.iter().map(ToString::to_string).collect()
}

#[derive(Debug, Clone, Copy)]
pub struct Reached(SocketAddr);

impl Reached {
    /// Compares addresses, not their spelling, so that an IPv4 request to a socket on `::`
    /// matches its mapped address.
    pub fn is_named_by(self, host: &str) -> bool {
        if host.eq_ignore_ascii_case(&format!("localhost:{}", self.0.port())) {
            return true;
        }
        host.parse::<SocketAddr>().is_ok_and(|named| {
            named.port() == self.0.port() && named.ip().to_canonical() == self.0.ip().to_canonical()
        })
    }
}

/// A connection the page server accepted.
pub struct Connection {
    stream: TcpStream,
    reached: Reached,
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

    pub fn reached(&self) -> Reached {
        self.reached
    }
}

/// The sockets the page server listens on, the first on the loopback interface or on every
/// address, and the addresses it waits for the machine to have.
pub struct Listener {
    first: TcpListener,
    others: Vec<TcpListener>,
    missing: Vec<SocketAddr>,
}

/// Listens at `port` on every address of the machine where `addresses` names `0.0.0.0`, and
/// otherwise on the loopback interface and on each of `addresses`. An address the machine does
/// not have yet is left for [`Listener::serve`] to try again.
///
/// # Errors
///
/// The port cannot be listened on, as when another program holds it.
pub fn listen(port: u16, addresses: &[Address]) -> io::Result<Listener> {
    let loopback = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    if let Some(every) = every_address(addresses) {
        // On macOS, the `SO_REUSEADDR` the standard library sets lets a socket listen on every
        // address beside another program's on the loopback address alone; listening there
        // first finds that program.
        drop(bind(loopback)?);
        return Ok(Listener {
            first: bind(SocketAddr::new(every, port))?,
            others: Vec::new(),
            missing: Vec::new(),
        });
    }
    let mut named: Vec<SocketAddr> = addresses
        .iter()
        .map(|named| SocketAddr::new(named.0, port))
        .filter(|address| *address != loopback)
        .collect();
    named.sort_unstable();
    named.dedup();
    let first = bind(loopback)?;
    let (mut others, mut missing) = (Vec::new(), Vec::new());
    for address in named {
        match bind(address) {
            Ok(socket) => others.push(socket),
            Err(error) if error.kind() == io::ErrorKind::AddrNotAvailable => missing.push(address),
            Err(error) => return Err(error),
        }
    }
    Ok(Listener {
        first,
        others,
        missing,
    })
}

/// The address that names every address, `0.0.0.0` before `::` whatever order they are named in,
/// so that the same addresses named in another order are listened on and recorded alike.
fn every_address(addresses: &[Address]) -> Option<IpAddr> {
    addresses
        .iter()
        .map(|named| named.0)
        .filter(IpAddr::is_unspecified)
        .min()
}

/// The one place the page server takes a socket.
fn bind(address: SocketAddr) -> io::Result<TcpListener> {
    TcpListener::bind(address)
}

impl Listener {
    /// Takes each connection received on any of its sockets and hands it to `serve` on a thread
    /// of its own, and tries each address the machine did not have again every second, through
    /// `clock`, until it listens there too. It returns only where the first socket fails.
    pub fn serve<F>(self, clock: &'static (impl Clock + Sync), serve: F)
    where
        F: Fn(Connection) + Send + Sync + Clone + 'static,
    {
        let Self {
            first,
            others,
            missing,
        } = self;
        if !missing.is_empty() {
            let serve = serve.clone();
            thread::spawn(move || {
                take_missing(missing, bind, clock, |socket| accept_apart(socket, &serve));
            });
        }
        for socket in others {
            accept_apart(socket, &serve);
        }
        accept(&first, &serve);
    }
}

fn accept_apart<F>(socket: TcpListener, serve: &F)
where
    F: Fn(Connection) + Send + Sync + Clone + 'static,
{
    let serve = serve.clone();
    thread::spawn(move || accept(&socket, &serve));
}

/// A connection whose address on the machine cannot be read is closed, since its `Host` could
/// not be checked.
fn accept<F>(socket: &TcpListener, serve: &F)
where
    F: Fn(Connection) + Send + Sync + Clone + 'static,
{
    for stream in socket.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let Ok(reached) = stream.local_addr() else {
            continue;
        };
        let _ = stream.set_read_timeout(Some(READING));
        let serve = serve.clone();
        let reached = Reached(reached);
        thread::spawn(move || serve(Connection { stream, reached }));
    }
}

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
    use loft_ports::clock::SystemClock;

    use super::*;

    #[test]
    fn an_address_the_machine_gains_is_listened_on_within_5_seconds() {
        let appears = Duration::from_millis(3_500);
        let clock = SystemClock::with_epoch(Some("0")).passing_if(true);
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

    #[test]
    fn the_addresses_recorded_leave_out_the_loopback_address_and_any_beside_every_address() {
        let named = |texts: &[&str]| -> Vec<Address> {
            texts
                .iter()
                .map(|text| parse_address(text).unwrap())
                .collect()
        };
        assert_eq!(recorded(&[]), Vec::<String>::new());
        assert_eq!(recorded(&named(&["127.0.0.1"])), Vec::<String>::new());
        assert_eq!(
            recorded(&named(&[
                "192.168.1.5",
                "100.64.0.1",
                "127.0.0.1",
                "100.64.0.1"
            ])),
            ["100.64.0.1", "192.168.1.5"]
        );
        assert_eq!(recorded(&named(&["100.64.0.1", "0.0.0.0"])), ["0.0.0.0"]);
        assert_eq!(recorded(&named(&["::", "0.0.0.0"])), ["0.0.0.0"]);
    }

    #[test]
    fn a_host_names_the_address_reached_by_its_ip_address_or_localhost_at_its_port() {
        let reached = Reached("192.168.1.5:7411".parse().unwrap());
        assert!(reached.is_named_by("192.168.1.5:7411"));
        assert!(reached.is_named_by("LOCALHOST:7411"));
        assert!(!reached.is_named_by("192.168.1.5:7412"));
        assert!(!reached.is_named_by("127.0.0.1:7411"));
        assert!(!reached.is_named_by("machine.example.ts.net:7411"));
        let mapped = Reached("[::ffff:192.168.1.5]:7411".parse().unwrap());
        assert!(mapped.is_named_by("192.168.1.5:7411"));
        assert!(Reached("[::1]:7411".parse().unwrap()).is_named_by("[::1]:7411"));
    }
}
