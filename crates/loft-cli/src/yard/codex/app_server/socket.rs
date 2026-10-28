//! Local transport selection; unsupported platforms retain ordinary native hook activity.

use std::io;
use std::path::Path;

#[cfg(unix)]
pub type Stream = std::os::unix::net::UnixStream;

#[cfg(unix)]
pub fn connect(path: &Path) -> io::Result<Stream> {
    use socket2::{Domain, SockAddr, Socket, Type};
    use std::os::fd::OwnedFd;
    use std::time::Duration;

    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    socket.connect_timeout(&SockAddr::unix(path)?, Duration::from_millis(50))?;
    let descriptor: OwnedFd = socket.into();
    let stream = Stream::from(descriptor);
    stream.set_nonblocking(true)?;
    Ok(stream)
}

#[cfg(not(unix))]
pub struct Stream;

#[cfg(not(unix))]
pub fn connect(_path: &Path) -> io::Result<Stream> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

#[cfg(not(unix))]
impl io::Read for Stream {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(not(unix))]
impl io::Write for Stream {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}
