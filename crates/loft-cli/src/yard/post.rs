//! Posting to the page server that runs, as an adapter posts a session's events: with the control
//! token of `~/.yard/token` and no `Origin`, through the network client, which reaches the
//! loopback interface alone. A post that finds no token or no answer is lost, and says so.

use serde_json::Value;
use yardlog_store::home::{Home, HomeFile};

/// Posts `body` to `path` of the page server on `port`; whether it accepted it.
pub fn post(home: &Home, port: u16, path: &str, body: &Value) -> bool {
    let Ok(Some(token)) = home.read(HomeFile::Token) else {
        return false;
    };
    let token = String::from_utf8_lossy(&token);
    crate::network::post_local_json(port, path, token.trim(), body.to_string().as_bytes())
}
