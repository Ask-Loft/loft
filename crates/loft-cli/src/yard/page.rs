//! Where the branch's Yard Log is on the page. A command learns whether a page server runs from
//! `~/.yard/server.json`, which the running server writes, and from the lock the server holds
//! on `~/.yard/server.lock` for as long as its process runs: the operating system frees the
//! lock when the process ends, however it ends, so a file left by a server that stopped names
//! no running one. The command opens no connection.

use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Value, json};
use yardlog_store::home::{Home, HomeFile};
use yardlog_store::lock;

/// The lock a running page server holds, in `~/.yard/`.
pub const SERVER_LOCK: &str = "server.lock";

/// The port of the page server that runs, where one does.
fn running_port(home: &Home) -> Option<u16> {
    let bytes = home.read(HomeFile::Server).ok()??;
    let server: Value = serde_json::from_slice(&bytes).ok()?;
    let port = u16::try_from(server["port"].as_u64()?).ok()?;
    let held = home.dir().join(SERVER_LOCK);
    // A lock file that is not there is held by no server; looking does not create it.
    if !held.is_file() || !lock::is_held(&held).ok()? {
        return None;
    }
    Some(port)
}

/// The address of `branch`'s Yard Log in `tree` on the page, or, where no page server runs, that
/// the page is unavailable, with the port it would listen on.
pub fn address(tree: &Path, branch: &str, port: u16) -> Value {
    match Home::from_env().as_ref().and_then(running_port) {
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
