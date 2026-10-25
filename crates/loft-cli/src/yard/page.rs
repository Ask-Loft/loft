//! Where the branch's Yard Log is on the page. A command learns whether a page server runs from
//! `~/.yard/server.json`, which the running server writes, and from the lock the server holds
//! on `~/.yard/server.lock` for as long as its process runs: the operating system frees the
//! lock when the process ends, however it ends, so a file left by a server that stopped names
//! no running one. The command opens no connection.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::OnceLock;

use serde_json::{Value, json};
use yardlog_store::YardStoreError;
use yardlog_store::home::{Home, HomeFile};
use yardlog_store::lock;

/// The lock a running page server holds, in `~/.yard/`.
pub const SERVER_LOCK: &str = "server.lock";

/// The port this process serves the page on, where it is the page server.
static SERVING: OnceLock<u16> = OnceLock::new();

/// Records that this process serves the page on `port`, so that what it answers names its own
/// address without looking for a page server.
pub fn serving(port: u16) {
    let _ = SERVING.set(port);
}

/// The page server `~/.yard/server.json` names, while it holds the lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    pub port: u16,
    pub version: String,
}

/// The page server that runs, where one does: the lock on `~/.yard/server.lock` is held and
/// `~/.yard/server.json` names it.
///
/// # Errors
///
/// The lock or the file cannot be read.
pub fn running(home: &Home) -> Result<Option<Running>, YardStoreError> {
    let held = home.dir().join(SERVER_LOCK);
    // A lock file that is not there is held by no server; looking does not create it.
    if !held.is_file() || !lock::is_held(&held)? {
        return Ok(None);
    }
    let Some(bytes) = home.read(HomeFile::Server)? else {
        return Ok(None);
    };
    let Ok(server) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(None);
    };
    let port = server["port"]
        .as_u64()
        .and_then(|port| u16::try_from(port).ok());
    let version = server["version"].as_str().map(ToOwned::to_owned);
    Ok(port
        .zip(version)
        .map(|(port, version)| Running { port, version }))
}

/// The address of `branch`'s Yard Log in `tree` on the page, or, where no page server runs, that
/// the page is unavailable, with the port it would listen on.
pub fn address(tree: &Path, branch: &str, port: u16) -> Value {
    let running = SERVING.get().copied().or_else(|| {
        let home = Home::from_env()?;
        running(&home).ok().flatten().map(|server| server.port)
    });
    match running {
        Some(port) => json!({
            "available": true,
            "port": port,
            "url": url(port, tree, branch),
        }),
        None => json!({ "available": false, "port": port }),
    }
}

/// The page's address for a branch's Yard Log in a tree.
pub fn url(port: u16, tree: &Path, branch: &str) -> String {
    format!(
        "http://127.0.0.1:{port}/#tree={}&branch={}",
        encode(&tree.display().to_string()),
        encode(branch)
    )
}

/// `text` with every byte but the unreserved ones percent-encoded, as a URL's fragment holds it.
fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}
